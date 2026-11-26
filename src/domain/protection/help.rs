//! Where to find help (#159). A red light or a refusal with no way forward
//! leaves the family alone, so every refusal, every summary with red results
//! and the review report name the free paths, plus any local contact the
//! project keeps.

/// One way to get help.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HelpPath {
    pub name: &'static str,
    /// What it offers, in plain words.
    pub what: &'static str,
    /// How to reach it.
    pub how: &'static str,
}

/// The free help paths, in the order a family should try them.
pub const HELP_PATHS: [HelpPath; 3] = [
    HelpPath {
        name: "Assistência técnica gratuita (ATHIS)",
        what: "A Lei federal nº 11.888/2008 garante às famílias com renda de até três salários mínimos assistência técnica pública e gratuita para projetar e construir a própria moradia.",
        how: "Pergunte na prefeitura (secretaria de habitação ou de obras) e no CAU (arquitetos) ou no CREA (engenheiros) do seu estado. Leve o memorial do Trazo.",
    },
    HelpPath {
        name: "Escritórios modelo das universidades",
        what: "Cursos de arquitetura e de engenharia que atendem a comunidade de graça ou a baixo custo.",
        how: "Procure a universidade mais próxima e pergunte pelo escritório modelo.",
    },
    HelpPath {
        name: "Defesa Civil (199)",
        what: "Risco de desabamento, deslizamento ou enchente: terreno em encosta, perto de córrego ou em aterro, trincas na casa ou nas casas vizinhas.",
        how: "Ligue 199.",
    },
];

/// One sentence that points to help, for a refusal or a red result.
pub const HELP_SENTENCE: &str = "Se não puder pagar um profissional, peça assistência técnica gratuita (ATHIS, Lei 11.888/2008) na prefeitura, no CAU ou no CREA; em risco de desabamento ou deslizamento, ligue para a Defesa Civil (199).";

/// The help paths and the project's own local contacts, one line each.
pub fn help_lines(contacts: &[String]) -> Vec<String> {
    HELP_PATHS
        .iter()
        .map(|path| format!("{}: {} {}", path.name, path.what, path.how))
        .chain(
            contacts
                .iter()
                .filter(|c| !c.trim().is_empty())
                .map(|c| format!("Contato local: {}", c.trim())),
        )
        .collect()
}

/// Cleans the local contacts typed by the person: one per line, trimmed,
/// without blanks, at most 12 of up to 200 characters.
pub fn clean_contacts(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| line.chars().take(200).collect())
        .take(12)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn help_names_free_assistance_and_the_civil_defence() {
        let lines = help_lines(&["Prefeitura: (11) 0000-0000".to_owned(), "  ".to_owned()]);
        assert_eq!(lines.len(), HELP_PATHS.len() + 1);
        assert!(lines.iter().any(|l| l.contains("11.888/2008")));
        assert!(lines.iter().any(|l| l.contains("199")));
        assert!(HELP_SENTENCE.contains("ATHIS"));
    }

    #[test]
    fn contacts_are_trimmed_and_limited() {
        let typed = format!("  a  \n\n b\n{}", "x".repeat(300));
        let contacts = clean_contacts(&typed);
        assert_eq!(contacts.len(), 3);
        assert_eq!(contacts[0], "a");
        assert_eq!(contacts[2].chars().count(), 200);
    }
}
