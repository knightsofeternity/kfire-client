//! Le fichier qui fait ouvrir à Rocket League sa socket de statistiques.
//!
//! Sans lui il n'y a RIEN à lire. Il n'est écrit qu'après accord du membre,
//! ayant vu le chemin exact et le contenu exact, et retirer le suivi le retire.
//!
//! ATTENTION : le README de `ke-rl-tracker` décrit une section `[StatsAPI]`
//! avec `bEnabled` et `ListenPort`. C'est FAUX. Le code qui tourne chez son
//! auteur écrit ce qui suit, et c'est lui qui fait foi.

/// Le port par défaut de la socket de statistiques.
pub const DEFAULT_PORT: u16 = 49123;

/// La section que le jeu lit pour décider d'exporter ses statistiques.
///
/// PacketSendRate accepte 30, 60, 90 ou 120. On demande le MINIMUM : on agrège
/// localement, donc trente états par seconde suffisent largement, et on prend
/// au jeu moins de temps machine que le traqueur d'origine, qui en demandait
/// soixante.
pub const STATS_BLOCK: &str = "\n[TAGame.MatchStatsExporter_TA]\nPort=49123\nPacketSendRate=30\n";

/// Le nom de la section, en minuscules, pour une comparaison insensible à la
/// casse.
const SECTION: &str = "[tagame.matchstatsexporter_ta]";

/// Si le fichier demande déjà l'export, le nôtre ou celui d'un autre traqueur.
pub fn has_stats_section(contents: &str) -> bool {
    contents
        .lines()
        .any(|l| l.trim().to_ascii_lowercase() == SECTION)
}

/// Le contenu avec notre section ajoutée, ou inchangé s'il y en a déjà une.
///
/// Ajouter une SECONDE section ne serait pas fusionné par le jeu, donc une
/// section existante, très probablement celle d'un autre traqueur, est laissée
/// strictement tranquille.
pub fn with_stats_block(contents: &str) -> String {
    if has_stats_section(contents) {
        return contents.to_string();
    }
    format!("{contents}{STATS_BLOCK}")
}

/// Le contenu sans la section, à condition qu'elle soit la nôtre.
///
/// Le retrait se fait ligne à ligne et non par correspondance exacte, parce
/// qu'un fichier ouvert dans le Bloc-notes revient en CRLF. Avec une
/// correspondance exacte, désactiver le suivi ne ferait alors RIEN : l'écran
/// confirmerait au membre que c'est coupé pendant que le jeu continuerait
/// d'exporter. Quelqu'un qui demande à ne plus être suivi doit vraiment cesser
/// de l'être.
///
/// On ne retire que ce qu'on a écrit : casser la configuration d'un autre
/// traqueur en se désactivant serait inacceptable. La section n'est reconnue
/// comme nôtre que si ses réglages sont exactement les nôtres.
pub fn without_stats_block(contents: &str) -> String {
    let eol = if contents.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let lines: Vec<&str> = contents
        .split('\n')
        .map(|l| l.trim_end_matches('\r'))
        .collect();

    let Some(start) = lines
        .iter()
        .position(|l| l.trim().to_ascii_lowercase() == SECTION)
    else {
        return contents.to_string();
    };
    // La section court jusqu'à la suivante, ou jusqu'à la fin.
    let end = lines[start + 1..]
        .iter()
        .position(|l| l.trim().starts_with('['))
        .map(|i| start + 1 + i)
        .unwrap_or(lines.len());

    if !is_ours(&lines[start + 1..end]) {
        return contents.to_string();
    }

    let mut kept: Vec<&str> = Vec::with_capacity(lines.len());
    kept.extend_from_slice(&lines[..start]);
    kept.extend_from_slice(&lines[end..]);
    // La ligne vide que notre bloc avait ajoutée avant lui repart avec.
    while kept.last().is_some_and(|l| l.trim().is_empty()) {
        kept.pop();
    }
    if kept.is_empty() {
        return String::new();
    }
    let mut out = kept.join(eol);
    out.push_str(eol);
    out
}

/// Si les réglages de cette section sont exactement ceux que nous écrivons.
///
/// C'est ce qui distingue notre bloc de celui d'un autre traqueur, maintenant
/// que la comparaison n'est plus littérale.
fn is_ours(body: &[&str]) -> bool {
    let mut port = false;
    let mut rate = false;
    for l in body {
        let t = l.trim();
        if t.is_empty() {
            continue;
        }
        let Some((k, v)) = t.split_once('=') else {
            return false;
        };
        match (k.trim().to_ascii_lowercase().as_str(), v.trim()) {
            ("port", "49123") => port = true,
            ("packetsendrate", "30") => rate = true,
            _ => return false,
        }
    }
    port && rate
}

/// Le port que le jeu écoutera, d'après le fichier.
///
/// Si une section est déjà là, c'est SON port qui vaut, pas le nôtre : c'est
/// celui-là que le jeu ouvrira. Une valeur illisible ou hors des ports
/// utilisateurs retombe sur le défaut.
pub fn port_in(contents: &str) -> u16 {
    for line in contents.lines() {
        let l = line.trim();
        let Some((k, v)) = l.split_once('=') else {
            continue;
        };
        if !k.trim().eq_ignore_ascii_case("port") {
            continue;
        }
        if let Ok(p) = v.trim().parse::<u32>() {
            if (1024..=65535).contains(&p) {
                return p as u16;
            }
        }
    }
    DEFAULT_PORT
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_file_gets_the_whole_block() {
        let out = with_stats_block("");
        assert!(out.contains("[TAGame.MatchStatsExporter_TA]"));
        assert!(out.contains("Port=49123"));
        assert!(out.contains("PacketSendRate=30"));
    }

    #[test]
    fn an_existing_section_is_left_strictly_alone() {
        // Très probablement celle d'un autre traqueur. On n'y touche pas, et
        // surtout on n'ajoute pas une SECONDE section : le jeu ne fusionne pas.
        let theirs = "[TAGame.MatchStatsExporter_TA]\nPort=50000\nPacketSendRate=120\n";
        assert_eq!(with_stats_block(theirs), theirs);
    }

    #[test]
    fn other_sections_survive() {
        let other = "[SomethingElse]\nKey=1\n";
        let out = with_stats_block(other);
        assert!(out.starts_with(other));
        assert!(out.contains("[TAGame.MatchStatsExporter_TA]"));
    }

    #[test]
    fn removing_takes_back_only_our_block() {
        let other = "[SomethingElse]\nKey=1\n";
        let with = with_stats_block(other);
        assert_eq!(without_stats_block(&with).trim_end(), other.trim_end());
    }

    #[test]
    fn removing_leaves_someone_elses_block_alone() {
        // On ne retire que ce qu'on a écrit. Casser la configuration d'un autre
        // traqueur en se désactivant serait inacceptable.
        let theirs = "[TAGame.MatchStatsExporter_TA]\nPort=50000\nPacketSendRate=120\n";
        assert_eq!(without_stats_block(theirs), theirs);
    }

    #[test]
    fn the_port_is_read_from_the_file_when_it_is_there() {
        let theirs = "[TAGame.MatchStatsExporter_TA]\nPort=50000\nPacketSendRate=120\n";
        assert_eq!(port_in(theirs), 50000);
    }

    #[test]
    fn the_port_falls_back_to_the_default() {
        assert_eq!(port_in(""), DEFAULT_PORT);
        assert_eq!(
            port_in("[TAGame.MatchStatsExporter_TA]\nPacketSendRate=60\n"),
            DEFAULT_PORT
        );
    }

    #[test]
    fn a_port_with_spaces_and_case_is_still_read() {
        assert_eq!(
            port_in("[TAGame.MatchStatsExporter_TA]\n port = 49999 \n"),
            49999
        );
    }

    #[test]
    fn an_absurd_port_is_refused_and_falls_back() {
        assert_eq!(
            port_in("[TAGame.MatchStatsExporter_TA]\nPort=0\n"),
            DEFAULT_PORT
        );
        assert_eq!(
            port_in("[TAGame.MatchStatsExporter_TA]\nPort=99999999\n"),
            DEFAULT_PORT
        );
    }

    #[test]
    fn detecting_our_own_block_is_exact() {
        assert!(!has_stats_section(""));
        assert!(has_stats_section("[TAGame.MatchStatsExporter_TA]\n"));
        assert!(has_stats_section("x\n[tagame.matchstatsexporter_ta]\ny\n"));
    }

    #[test]
    fn removing_works_on_a_file_that_came_back_from_notepad() {
        // Le Bloc-notes réécrit tout le fichier en CRLF. Avec une
        // correspondance exacte, le retrait ne faisait rien du tout.
        let unix = with_stats_block("[SomethingElse]\nKey=1\n");
        let crlf = unix.replace('\n', "\r\n");
        let out = without_stats_block(&crlf);
        assert!(
            !has_stats_section(&out),
            "notre bloc devait partir : {out:?}"
        );
        assert!(out.contains("Key=1"), "le reste du fichier devait rester");
    }

    #[test]
    fn removing_a_crlf_file_keeps_it_in_crlf() {
        let crlf = with_stats_block("[SomethingElse]\nKey=1\n").replace('\n', "\r\n");
        let out = without_stats_block(&crlf);
        assert!(
            out.contains("\r\n"),
            "les fins de ligne du membre sont les siennes"
        );
        assert!(!out.contains("\n\n"), "pas de ligne vide parasite");
    }

    #[test]
    fn removing_still_leaves_someone_elses_block_alone_in_crlf_too() {
        let theirs = "[TAGame.MatchStatsExporter_TA]\r\nPort=50000\r\nPacketSendRate=120\r\n";
        assert_eq!(without_stats_block(theirs), theirs);
    }

    #[test]
    fn a_section_with_an_extra_key_is_not_ours() {
        // Quelqu'un a ajouté un réglage au nôtre : ce n'est plus le nôtre, on
        // n'y touche pas.
        let mixed =
            "[TAGame.MatchStatsExporter_TA]\nPort=49123\nPacketSendRate=30\nSomethingElse=1\n";
        assert_eq!(without_stats_block(mixed), mixed);
    }

    #[test]
    fn removing_our_block_from_a_file_that_holds_nothing_else_empties_it() {
        let only_ours = with_stats_block("");
        assert_eq!(without_stats_block(&only_ours), "");
    }
}
