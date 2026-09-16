//! Suivi des matchs Rocket League : le jeu n'a pas d'API joueur publique, donc
//! le client de bureau lit la socket de statistiques qu'il ouvre en local et
//! n'en rapporte qu'un résumé.
//!
//! Ce qui NE quitte JAMAIS cette machine, par conception : le nom des autres
//! joueurs du match, coéquipiers comme adversaires. Le flux les porte tous. Le
//! client s'en sert pour calculer, puis n'émet que des faits sur le membre.

pub mod config;
pub mod frames;
pub mod parser;
pub mod paths;
