//! L'accès à l'API locale de Riot, et rien d'autre.
//!
//! Le jeu ouvre cette API sur la machine du joueur, et UNIQUEMENT pendant une
//! partie. Elle n'exige aucune clé : elle n'est joignable que depuis la machine
//! où le jeu tourne.

/// L'adresse de l'API locale, pour les journaux.
pub const BASE: &str = "https://127.0.0.1:2999";

/// Tout l'état de la partie en une requête.
pub const ALL_GAME_DATA: &str = "https://127.0.0.1:2999/liveclientdata/allgamedata";

/// Le jeu répond parfois vite, mais il n'a aucune raison de nous faire
/// attendre : hors partie la connexion est refusée tout de suite, et pendant
/// une partie l'API est locale.
const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);

/// Le client HTTP dédié à l'API locale du jeu, et à elle seule.
///
/// POURQUOI il accepte un certificat invalide : le jeu présente un certificat
/// auto-signé, signé par une autorité que personne d'autre ne connaît. Sur
/// `127.0.0.1` il n'y a aucun réseau entre nous et le jeu, donc aucun tiers à
/// qui la vérification du certificat servirait de protection : elle ne
/// protègerait de rien et empêcherait seulement de lire.
///
/// POURQUOI il ne doit JAMAIS servir à autre chose : réutiliser ce client pour
/// parler au serveur KFIRE, ou à quoi que ce soit qui sorte de cette machine,
/// supprimerait une vraie protection, celle qui empêche un intermédiaire de se
/// faire passer pour le serveur. Le client des échanges avec KFIRE est dans
/// `crate::api` et vérifie les certificats ; celui-ci reste enfermé ici.
pub fn client() -> Option<reqwest::Client> {
    reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .timeout(TIMEOUT)
        // Aucun proxy : la requête ne doit jamais sortir de la machine, même si
        // l'environnement en déclare un pour le reste du monde.
        .no_proxy()
        .build()
        .inspect_err(|e| log::warn!("lol: could not build the local API client: {e}"))
        .ok()
}

/// Rend la réponse brute d'`allgamedata`, ou `None` s'il n'y a pas de partie.
///
/// L'échec est le cas NORMAL : hors partie l'API n'existe pas, et on sonde en
/// boucle. Rien n'est journalisé au-dessus de `debug`, sinon le journal se
/// remplirait d'avertissements identiques à chaque seconde passée au menu.
pub async fn fetch_all_game_data(client: &reqwest::Client) -> Option<String> {
    match client.get(ALL_GAME_DATA).send().await {
        Ok(r) if r.status().is_success() => r
            .text()
            .await
            .inspect_err(|e| log::debug!("lol: the live API cut the answer short: {e}"))
            .ok(),
        // L'API répond 404 sur une partie qui n'a pas encore commencé.
        Ok(r) => {
            log::debug!("lol: the live API answered {}", r.status());
            None
        }
        Err(e) => {
            log::debug!("lol: no live API right now ({e})");
            None
        }
    }
}
