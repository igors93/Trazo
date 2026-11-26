//! Format-2 XML (and JSON) export/import for building code packs (T-08).
//!
//! Zero external dependencies, zero panics, lenient forward-compatible import
//! collecting unknown disciplines/sections/keys in `left_out`.

use std::collections::BTreeMap;

use serde_json::{Map, Value};

use super::{
    BuildingCode, Citation, CodeDiscipline, DocumentRef, MAX_XML_BYTES, RuleKind, RuleSource,
};

/// Result of importing one or more code packs from XML or JSON.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ImportedPacks {
    pub codes: Vec<BuildingCode>,
    pub left_out: Vec<String>,
}

const MAX_LEFT_OUT: usize = 100;

/// Export a single [`BuildingCode`] to Format-2 XML.
pub fn export_code_xml(code: &BuildingCode) -> String {
    let mut out = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    write_code_element(&mut out, code, "");
    out
}

/// Export multiple [`BuildingCode`] packs (e.g. a multi-discipline or layered bundle)
/// to Format-2 XML wrapped in `<building-codes format="2">`.
pub fn export_bundle_xml(codes: &[BuildingCode]) -> String {
    if codes.len() == 1
        && let Some(first) = codes.first()
    {
        return export_code_xml(first);
    }
    let mut out =
        String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<building-codes format=\"2\">\n");
    for code in codes {
        write_code_element(&mut out, code, "  ");
    }
    out.push_str("</building-codes>\n");
    out
}

/// Export a single [`BuildingCode`] or bundle to pretty JSON.
pub fn export_bundle_json(codes: &[BuildingCode]) -> Result<String, String> {
    if codes.len() == 1
        && let Some(first) = codes.first()
    {
        serde_json::to_string_pretty(first).map_err(|e| format!("Falha ao serializar JSON: {e}"))
    } else {
        serde_json::to_string_pretty(codes).map_err(|e| format!("Falha ao serializar JSON: {e}"))
    }
}

/// Import one or more [`BuildingCode`] packs from JSON (object or array).
pub fn import_json(raw: &str) -> Result<ImportedPacks, String> {
    if raw.len() > MAX_XML_BYTES {
        return Err("Arquivo de norma excede o limite de 16 MiB.".to_owned());
    }
    let trimmed = raw.trim();
    if trimmed.starts_with('[') {
        let codes: Vec<BuildingCode> =
            serde_json::from_str(trimmed).map_err(|e| format!("JSON de normas inválido: {e}"))?;
        Ok(ImportedPacks {
            codes,
            left_out: Vec::new(),
        })
    } else {
        let code: BuildingCode =
            serde_json::from_str(trimmed).map_err(|e| format!("JSON de norma inválido: {e}"))?;
        Ok(ImportedPacks {
            codes: vec![code],
            left_out: Vec::new(),
        })
    }
}

/// Auto-detect XML vs JSON and import code packs.
pub fn import_auto(raw: &str) -> Result<ImportedPacks, String> {
    let trimmed = raw.trim_start();
    if trimmed.starts_with('<') {
        import_xml(trimmed)
    } else {
        import_json(trimmed)
    }
}

fn write_code_element(out: &mut String, code: &BuildingCode, indent: &str) {
    out.push_str(indent);
    out.push_str("<building-code format=\"2\"");
    push_attr(out, "code", &code.code);
    push_attr(out, "discipline", code.discipline.key());
    push_attr(out, "name", &code.name);
    if !code.region.is_empty() {
        push_attr(out, "region", &code.region);
    }
    if !code.edition.is_empty() {
        push_attr(out, "edition", &code.edition);
    }
    if !code.based_on.is_empty() {
        push_attr(out, "based-on", &code.based_on);
    }
    if !code.amends.is_empty() {
        push_attr(out, "amends", &code.amends);
    }
    if code.is_checked() {
        push_attr(out, "checked", "true");
    }
    out.push_str(">\n");

    let inner = format!("{indent}  ");
    if !code.note.is_empty() {
        write_text_tag(out, &inner, "note", &code.note);
    }
    for reference in &code.references {
        write_text_tag(out, &inner, "reference", reference);
    }
    if !code.checked.is_empty() {
        write_text_tag(out, &inner, "checked", &code.checked);
    }
    for (id, doc) in &code.documents {
        out.push_str(&inner);
        out.push_str("<document");
        push_attr(out, "id", id);
        push_attr(out, "title", &doc.title);
        if !doc.edition.is_empty() {
            push_attr(out, "edition", &doc.edition);
        }
        if !doc.publisher.is_empty() {
            push_attr(out, "publisher", &doc.publisher);
        }
        if !doc.url.is_empty() {
            push_attr(out, "url", &doc.url);
        }
        if !doc.accessed.is_empty() {
            push_attr(out, "accessed", &doc.accessed);
        }
        out.push_str("/>\n");
    }

    for (sec_name, sec_val) in &code.sections {
        out.push_str(&inner);
        out.push_str("<section");
        push_attr(out, "name", sec_name);
        if let Some(src) = code.sources.get(sec_name) {
            push_source_attrs(out, src);
        }
        if let Some(kind) = code.kinds.get(sec_name) {
            push_attr(out, "kind", kind.key());
        }
        if let Some(obj) = sec_val.as_object() {
            out.push_str(">\n");
            let rule_indent = format!("{inner}  ");
            let mut keys: Vec<&String> = obj.keys().collect();
            keys.sort();
            for key in keys {
                if let Some(val) = obj.get(key) {
                    out.push_str(&rule_indent);
                    out.push_str("<rule");
                    push_attr(out, "name", key);
                    push_value_attr(out, val);
                    let dotted = format!("{sec_name}.{key}");
                    if let Some(src) = code.sources.get(&dotted) {
                        push_source_attrs(out, src);
                    }
                    if let Some(kind) = code.kinds.get(&dotted) {
                        push_attr(out, "kind", kind.key());
                    }
                    out.push_str("/>\n");
                }
            }
            out.push_str(&inner);
            out.push_str("</section>\n");
        } else {
            if let Ok(serialized) = serde_json::to_string(sec_val) {
                push_attr(out, "json", &serialized);
            }
            out.push_str("/>\n");
        }
    }

    out.push_str(indent);
    out.push_str("</building-code>\n");
}

fn push_source_attrs(out: &mut String, src: &RuleSource) {
    match src {
        RuleSource::Text(t) => push_attr(out, "source", t),
        RuleSource::Cited(c) => {
            if !c.doc.is_empty() {
                push_attr(out, "source-doc", &c.doc);
            }
            if !c.item.is_empty() {
                push_attr(out, "source-item", &c.item);
            }
            if !c.page.is_empty() {
                push_attr(out, "source-page", &c.page);
            }
            if !c.note.is_empty() {
                push_attr(out, "source-note", &c.note);
            }
        }
    }
}

fn push_value_attr(out: &mut String, val: &Value) {
    match val {
        Value::Bool(b) => push_attr(out, "value", if *b { "true" } else { "false" }),
        Value::Number(n) => push_attr(out, "value", &n.to_string()),
        Value::String(s) if !looks_ambiguous_scalar(s) => push_attr(out, "value", s),
        _ => {
            if let Ok(serialized) = serde_json::to_string(val) {
                push_attr(out, "json", &serialized);
            }
        }
    }
}

fn looks_ambiguous_scalar(s: &str) -> bool {
    let t = s.trim();
    t.is_empty()
        || t == "true"
        || t == "false"
        || t == "null"
        || t.parse::<f64>().is_ok()
        || t.starts_with('[')
        || t.starts_with('{')
}

fn push_attr(out: &mut String, name: &str, value: &str) {
    out.push(' ');
    out.push_str(name);
    out.push_str("=\"");
    escape_xml_into(out, value);
    out.push('"');
}

fn write_text_tag(out: &mut String, indent: &str, tag: &str, text: &str) {
    out.push_str(indent);
    out.push('<');
    out.push_str(tag);
    out.push('>');
    escape_xml_into(out, text);
    out.push_str("</");
    out.push_str(tag);
    out.push_str(">\n");
}

fn escape_xml_into(out: &mut String, s: &str) {
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(ch),
        }
    }
}

fn unescape_xml(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(idx) = rest.find('&') {
        out.push_str(&rest[..idx]);
        let after = &rest[idx + 1..];
        if let Some(semi) = after.find(';') {
            let ent = &after[..semi];
            match ent {
                "amp" => out.push('&'),
                "lt" => out.push('<'),
                "gt" => out.push('>'),
                "quot" => out.push('"'),
                "apos" => out.push('\''),
                _ if ent.starts_with('#') => {
                    let parsed = if let Some(hex) =
                        ent.strip_prefix("#x").or_else(|| ent.strip_prefix("#X"))
                    {
                        u32::from_str_radix(hex, 16).ok()
                    } else {
                        ent[1..].parse::<u32>().ok()
                    };
                    if let Some(ch) = parsed.and_then(char::from_u32) {
                        out.push(ch);
                    } else {
                        out.push('&');
                        out.push_str(ent);
                        out.push(';');
                    }
                }
                _ => {
                    out.push('&');
                    out.push_str(ent);
                    out.push(';');
                }
            }
            rest = &after[semi + 1..];
        } else {
            out.push('&');
            rest = after;
        }
    }
    out.push_str(rest);
    out
}

/// Minimal zero-dep XML DOM node for Format-2 code pack parsing.
#[derive(Debug)]
struct XmlNode {
    tag: String,
    attrs: BTreeMap<String, String>,
    text: String,
    children: Vec<XmlNode>,
}

/// Import one or more [`BuildingCode`] packs from Format-2 XML.
pub fn import_xml(raw: &str) -> Result<ImportedPacks, String> {
    if raw.len() > MAX_XML_BYTES {
        return Err("Arquivo XML excede o limite de 16 MiB.".to_owned());
    }
    let root = parse_xml_root(raw)?;
    let mut result = ImportedPacks::default();
    match root.tag.as_str() {
        "building-code" => {
            if let Some(code) = parse_code_node(&root, &mut result.left_out)? {
                result.codes.push(code);
            }
        }
        "building-codes" => {
            for child in &root.children {
                if child.tag == "building-code" {
                    if let Some(code) = parse_code_node(child, &mut result.left_out)? {
                        result.codes.push(code);
                    }
                } else {
                    push_left_out(
                        &mut result.left_out,
                        format!("tag desconhecida <{}>", child.tag),
                    );
                }
            }
        }
        other => {
            return Err(format!(
                "Elemento raiz XML esperado <building-code> ou <building-codes>, encontrado <{other}>."
            ));
        }
    }
    Ok(result)
}

fn push_left_out(left_out: &mut Vec<String>, item: String) {
    if left_out.len() < MAX_LEFT_OUT {
        left_out.push(item);
    }
}

fn parse_code_node(
    node: &XmlNode,
    left_out: &mut Vec<String>,
) -> Result<Option<BuildingCode>, String> {
    let disc_raw = node
        .attrs
        .get("discipline")
        .map(String::as_str)
        .unwrap_or("architectural");
    let Some(discipline) = CodeDiscipline::parse(disc_raw) else {
        push_left_out(left_out, format!("disciplina desconhecida: {disc_raw}"));
        return Ok(None);
    };
    let name = node
        .attrs
        .get("name")
        .cloned()
        .unwrap_or_else(|| "Norma importada".to_owned());
    let code_key = node
        .attrs
        .get("code")
        .cloned()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| super::slugify_code(&name));

    let mut code = BuildingCode {
        code: code_key,
        discipline,
        name,
        region: node.attrs.get("region").cloned().unwrap_or_default(),
        edition: node.attrs.get("edition").cloned().unwrap_or_default(),
        superseded_by: String::new(),
        amends: node.attrs.get("amends").cloned().unwrap_or_default(),
        based_on: node.attrs.get("based-on").cloned().unwrap_or_default(),
        default: false,
        checked: String::new(),
        references: Vec::new(),
        note: String::new(),
        documents: BTreeMap::new(),
        sections: BTreeMap::new(),
        sources: BTreeMap::new(),
        kinds: BTreeMap::new(),
    };

    for child in &node.children {
        match child.tag.as_str() {
            "note" => code.note = child.text.trim().to_owned(),
            "checked" => code.checked = child.text.trim().to_owned(),
            "reference" => {
                let r = child.text.trim();
                if !r.is_empty() {
                    code.references.push(r.to_owned());
                }
            }
            "document" => {
                if let Some(id) = child.attrs.get("id") {
                    code.documents.insert(
                        id.clone(),
                        DocumentRef {
                            title: child.attrs.get("title").cloned().unwrap_or_default(),
                            edition: child.attrs.get("edition").cloned().unwrap_or_default(),
                            publisher: child.attrs.get("publisher").cloned().unwrap_or_default(),
                            url: child.attrs.get("url").cloned().unwrap_or_default(),
                            accessed: child.attrs.get("accessed").cloned().unwrap_or_default(),
                        },
                    );
                }
            }
            "section" => {
                let Some(sec_name) = child.attrs.get("name") else {
                    push_left_out(left_out, "seção sem atributo name".to_owned());
                    continue;
                };
                if let Some(src) = parse_source_attrs(&child.attrs) {
                    code.sources.insert(sec_name.clone(), src);
                }
                if let Some(kind) = child.attrs.get("kind").and_then(|k| RuleKind::parse(k)) {
                    code.kinds.insert(sec_name.clone(), kind);
                }
                if let Some(raw_json) = child.attrs.get("json") {
                    let val: Value = serde_json::from_str(raw_json)
                        .map_err(|e| format!("JSON inválido na seção '{sec_name}': {e}"))?;
                    code.sections.insert(sec_name.clone(), val);
                } else {
                    let mut map = Map::new();
                    for rule in &child.children {
                        if rule.tag != "rule" {
                            push_left_out(
                                left_out,
                                format!("tag <{}> ignorada em seção {sec_name}", rule.tag),
                            );
                            continue;
                        }
                        let Some(rule_name) = rule.attrs.get("name") else {
                            continue;
                        };
                        let val = parse_value_attrs(&rule.attrs)?;
                        map.insert(rule_name.clone(), val);
                        let dotted = format!("{sec_name}.{rule_name}");
                        if let Some(src) = parse_source_attrs(&rule.attrs) {
                            code.sources.insert(dotted.clone(), src);
                        }
                        if let Some(kind) = rule.attrs.get("kind").and_then(|k| RuleKind::parse(k))
                        {
                            code.kinds.insert(dotted, kind);
                        }
                    }
                    code.sections.insert(sec_name.clone(), Value::Object(map));
                }
            }
            other => {
                push_left_out(left_out, format!("elemento <{other}> ignorado"));
            }
        }
    }

    Ok(Some(code))
}

fn parse_source_attrs(attrs: &BTreeMap<String, String>) -> Option<RuleSource> {
    if let Some(text) = attrs.get("source") {
        return Some(RuleSource::Text(text.clone()));
    }
    let doc = attrs.get("source-doc").cloned().unwrap_or_default();
    let item = attrs.get("source-item").cloned().unwrap_or_default();
    let page = attrs.get("source-page").cloned().unwrap_or_default();
    let note = attrs.get("source-note").cloned().unwrap_or_default();
    if !doc.is_empty() || !item.is_empty() || !page.is_empty() || !note.is_empty() {
        Some(RuleSource::Cited(Citation {
            doc,
            item,
            page,
            note,
        }))
    } else {
        None
    }
}

fn parse_value_attrs(attrs: &BTreeMap<String, String>) -> Result<Value, String> {
    if let Some(raw_json) = attrs.get("json") {
        return serde_json::from_str(raw_json)
            .map_err(|e| format!("Valor json inválido em regra XML: {e}"));
    }
    if let Some(raw_val) = attrs.get("value") {
        if raw_val == "true" {
            return Ok(Value::Bool(true));
        }
        if raw_val == "false" {
            return Ok(Value::Bool(false));
        }
        if let Ok(i) = raw_val.parse::<i64>() {
            return Ok(Value::Number(i.into()));
        }
        if let Ok(f) = raw_val.parse::<f64>()
            && let Some(n) = serde_json::Number::from_f64(f)
        {
            return Ok(Value::Number(n));
        }
        return Ok(Value::String(raw_val.clone()));
    }
    Ok(Value::Null)
}

/// Simple iterative XML parser (no recursion overflow, no panics, supports comments and XML decl).
fn parse_xml_root(input: &str) -> Result<XmlNode, String> {
    let bytes = input.as_bytes();
    let mut pos = 0usize;
    let mut stack: Vec<XmlNode> = Vec::new();
    let mut root: Option<XmlNode> = None;

    while pos < bytes.len() {
        let rest = &input[pos..];
        let Some(lt_rel) = rest.find('<') else {
            if let Some(top) = stack.last_mut() {
                top.text.push_str(&unescape_xml(rest));
            }
            break;
        };
        if lt_rel > 0
            && let Some(top) = stack.last_mut()
        {
            top.text.push_str(&unescape_xml(&rest[..lt_rel]));
        }
        pos += lt_rel;
        let tag_rest = &input[pos..];

        // XML declaration or processing instruction `<?...?>`
        if tag_rest.starts_with("<?") {
            let end = tag_rest
                .find("?>")
                .ok_or_else(|| "Declaração XML não fechada.".to_owned())?;
            pos += end + 2;
            continue;
        }
        // Comment `<!--...-->`
        if tag_rest.starts_with("<!--") {
            let end = tag_rest
                .find("-->")
                .ok_or_else(|| "Comentário XML não fechado.".to_owned())?;
            pos += end + 3;
            continue;
        }
        // Closing tag `</name>`
        if let Some(after_slash) = tag_rest.strip_prefix("</") {
            let end = after_slash
                .find('>')
                .ok_or_else(|| "Tag de fechamento XML malformada.".to_owned())?;
            let close_name = after_slash[..end].trim();
            let popped = stack
                .pop()
                .ok_or_else(|| format!("Tag de fechamento inesperada </{close_name}>."))?;
            if popped.tag != close_name {
                return Err(format!(
                    "Tag de fechamento </{close_name}> não corresponde a <{}>.",
                    popped.tag
                ));
            }
            if let Some(parent) = stack.last_mut() {
                parent.children.push(popped);
            } else {
                root = Some(popped);
            }
            pos += 2 + end + 1;
            continue;
        }

        // Opening or self-closing tag `<name attr="val" ...>`
        let (tag_len, name, attrs, self_closing) = parse_open_tag(tag_rest)?;
        pos += tag_len;
        let node = XmlNode {
            tag: name,
            attrs,
            text: String::new(),
            children: Vec::new(),
        };
        if self_closing {
            if let Some(parent) = stack.last_mut() {
                parent.children.push(node);
            } else {
                root = Some(node);
            }
        } else {
            if stack.len() >= 32 {
                return Err("Profundidade máxima de XML excedida.".to_owned());
            }
            stack.push(node);
        }
    }

    if !stack.is_empty() {
        return Err("Documento XML contém tags abertas sem fechamento.".to_owned());
    }
    root.ok_or_else(|| "Documento XML vazio.".to_owned())
}

fn parse_open_tag(s: &str) -> Result<(usize, String, BTreeMap<String, String>, bool), String> {
    let bytes = s.as_bytes();
    let mut i = 1usize; // skip '<'
    while i < bytes.len() && bytes[i].is_ascii_whitespace() {
        i += 1;
    }
    let name_start = i;
    while i < bytes.len() && !bytes[i].is_ascii_whitespace() && bytes[i] != b'>' && bytes[i] != b'/'
    {
        i += 1;
    }
    if i == name_start {
        return Err("Nome de tag XML vazio.".to_owned());
    }
    let name = s[name_start..i].to_owned();
    let mut attrs = BTreeMap::new();

    loop {
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= bytes.len() {
            return Err(format!("Tag <{name}> não fechada."));
        }
        if bytes[i] == b'>' {
            return Ok((i + 1, name, attrs, false));
        }
        if bytes[i] == b'/' {
            if i + 1 < bytes.len() && bytes[i + 1] == b'>' {
                return Ok((i + 2, name, attrs, true));
            }
            return Err(format!("Barra inesperada na tag <{name}>."));
        }
        let attr_start = i;
        while i < bytes.len()
            && !bytes[i].is_ascii_whitespace()
            && bytes[i] != b'='
            && bytes[i] != b'>'
        {
            i += 1;
        }
        let attr_name = s[attr_start..i].to_owned();
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= bytes.len() || bytes[i] != b'=' {
            return Err(format!(
                "Esperado '=' após atributo '{attr_name}' em <{name}>."
            ));
        }
        i += 1;
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= bytes.len() || (bytes[i] != b'"' && bytes[i] != b'\'') {
            return Err(format!(
                "Valor de atributo '{attr_name}' sem aspas em <{name}>."
            ));
        }
        let quote = bytes[i];
        i += 1;
        let val_start = i;
        while i < bytes.len() && bytes[i] != quote {
            i += 1;
        }
        if i >= bytes.len() {
            return Err(format!(
                "Atributo '{attr_name}' sem aspa de fechamento em <{name}>."
            ));
        }
        let val = unescape_xml(&s[val_start..i]);
        i += 1;
        attrs.insert(attr_name, val);
    }
}
