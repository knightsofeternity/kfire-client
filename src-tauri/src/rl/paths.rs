//! Où Rocket League vit sur cette machine, et où il lit sa configuration.
//!
//! Contrairement à Hearthstone, dont le fichier de configuration vit dans
//! LOCALAPPDATA, celui-ci vit DANS le dossier d'installation. On ne peut donc
//! rien écrire tant qu'on n'a pas trouvé l'installation.

use std::path::PathBuf;

/// L'exécutable du jeu, tel que le scanner le voit.
pub const EXE: &str = "RocketLeague.exe";

/// Les emplacements d'installation courants, repris de `ke-rl-tracker` où ils
/// sont éprouvés en production. Sondés seulement si le process n'est pas là
/// pour nous dire la vérité.
pub const COMMON_DIRS: &[&str] = &[
    r"C:\Program Files (x86)\Steam\steamapps\common\rocketleague",
    r"C:\Program Files\Steam\steamapps\common\rocketleague",
    r"C:\Program Files\Epic Games\rocketleague",
    r"C:\Program Files (x86)\Epic Games\rocketleague",
];

/// Découpe le dernier élément d'un chemin, en traitant `/` ET `\` comme des
/// séparateurs, quelle que soit la plateforme hôte.
///
/// Un chemin Windows relevé sur la machine d'un membre garde les séparateurs de
/// cette machine. Confié à `std::path::Path` sur Linux, notre CI et cette
/// machine de développement, il ne serait pas découpé du tout, puisqu'Unix voit
/// `\` comme un caractère ordinaire. Même raison et même remède que dans
/// `hs/paths.rs`.
fn parent_str(s: &str) -> Option<&str> {
    let idx = s.rfind(['/', '\\'])?;
    Some(&s[..idx])
}

/// Le dossier d'installation qui contient cet exécutable.
///
/// Le jeu tourne depuis `<install>/Binaries/Win64/RocketLeague.exe`, donc
/// l'installation est trois crans au-dessus.
pub fn install_dir_of(exe: &str) -> Option<String> {
    let win64 = parent_str(exe)?;
    let binaries = parent_str(win64)?;
    let install = parent_str(binaries)?;
    if install.is_empty() {
        return None;
    }
    Some(install.to_string())
}

/// Le fichier de configuration à l'intérieur d'un dossier d'installation.
///
/// Le séparateur du dossier est conservé : on ne mélange pas les styles dans un
/// chemin qu'on va rendre au membre pour qu'il le reconnaisse.
pub fn config_path_in(install: &str) -> String {
    let sep = if install.contains('\\') { '\\' } else { '/' };
    format!("{install}{sep}TAGame{sep}Config{sep}DefaultStatsAPI.ini")
}

/// Le dossier d'installation déduit du process en cours, s'il tourne.
pub fn running_install_dir() -> Option<PathBuf> {
    use sysinfo::{ProcessRefreshKind, RefreshKind, System, UpdateKind};
    let sys = System::new_with_specifics(
        RefreshKind::nothing()
            .with_processes(ProcessRefreshKind::nothing().with_exe(UpdateKind::OnlyIfNotSet)),
    );
    for p in sys.processes().values() {
        if p.name().to_string_lossy().eq_ignore_ascii_case(EXE) {
            let exe = p.exe()?.to_string_lossy().to_string();
            return install_dir_of(&exe).map(PathBuf::from);
        }
    }
    None
}

/// Le premier emplacement courant qui existe réellement.
pub fn common_install_dir() -> Option<PathBuf> {
    COMMON_DIRS.iter().map(PathBuf::from).find(|d| d.is_dir())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_install_dir_is_three_levels_above_the_executable() {
        // Le jeu tourne depuis <install>/Binaries/Win64/RocketLeague.exe
        let exe = r"C:\Program Files (x86)\Steam\steamapps\common\rocketleague\Binaries\Win64\RocketLeague.exe";
        assert_eq!(
            install_dir_of(exe).as_deref(),
            Some(r"C:\Program Files (x86)\Steam\steamapps\common\rocketleague")
        );
    }

    #[test]
    fn a_windows_path_is_split_on_backslashes_even_on_unix() {
        // Ce test tourne sur Linux en CI. Path::parent n'y découperait rien.
        let exe = r"D:\Games\rocketleague\Binaries\Win64\RocketLeague.exe";
        assert_eq!(
            install_dir_of(exe).as_deref(),
            Some(r"D:\Games\rocketleague")
        );
    }

    #[test]
    fn a_unix_path_works_too() {
        let exe = "/home/x/rocketleague/Binaries/Win64/RocketLeague.exe";
        assert_eq!(install_dir_of(exe).as_deref(), Some("/home/x/rocketleague"));
    }

    #[test]
    fn a_path_too_shallow_yields_nothing() {
        assert_eq!(install_dir_of("RocketLeague.exe"), None);
        assert_eq!(install_dir_of(r"C:\RocketLeague.exe"), None);
    }

    #[test]
    fn the_config_path_hangs_off_the_install_dir() {
        assert_eq!(
            config_path_in(r"C:\Games\rocketleague"),
            r"C:\Games\rocketleague\TAGame\Config\DefaultStatsAPI.ini"
        );
    }

    #[test]
    fn a_unix_install_dir_keeps_unix_separators() {
        assert_eq!(
            config_path_in("/home/x/rocketleague"),
            "/home/x/rocketleague/TAGame/Config/DefaultStatsAPI.ini"
        );
    }
}
