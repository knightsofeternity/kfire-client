//! The gate that decides whether a state frame is allowed to start a match.
//!
//! Why it exists: the game reports the end of a match TWICE, `MatchEnded` when
//! the match ends and `MatchDestroyed` on the way back to the menu. Between the
//! two it keeps sending `UpdateState` frames for the end-of-match screen. The
//! watcher used to create a match out of any state frame, so those frames built
//! a second match carrying the same statistics with a clock restarted from
//! zero. In production 8 of a member's 15 recorded matches were such ghosts,
//! lasting 15 to 40 seconds where a Rocket League match lasts five minutes, and
//! they polluted his averages. GUID deduplication did not catch them; the
//! likeliest reason is that the post-match frames carry no `MatchGuid`, since
//! the dedup only applies when one is present, but that was inferred from the
//! stored rows and never confirmed against the game's own logs.
//!
//! The rule: once a match has ended, only an opening event may start the next
//! one, never a state frame.
//!
//! One hole stays open by design: before the FIRST end of a match after the
//! client starts, the gate is open, so a stray state frame could still invent a
//! match once per launch. That is the price of tracking a member who enables
//! tracking mid-game. The server refuses any match shorter than a minute, which
//! is what catches it.

pub struct MatchGate {
    may_start: bool,
}

impl MatchGate {
    pub fn new() -> Self {
        // Open at startup on purpose: a member who turns tracking on in the
        // middle of a game gets no opening event, and the first state frame is
        // his only chance to be tracked at all.
        Self { may_start: true }
    }

    /// An opening event arrived: a match is allowed to begin.
    pub fn opened(&mut self) {
        self.may_start = true;
    }

    /// May a state frame start a match, when none is in progress?
    pub fn may_start(&self) -> bool {
        self.may_start
    }

    /// The match is over.
    pub fn closed(&mut self) {
        // The frames that follow belong to the end-of-match screen, not to a
        // new match. Only an opening event reopens the gate.
        self.may_start = false;
    }
}

impl Default for MatchGate {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn au_depart_une_trame_detat_peut_demarrer_un_match() {
        // Rattrapage en cours de partie : le membre active le suivi au milieu
        // d'un match, aucun évènement d'ouverture ne viendra.
        let gate = MatchGate::new();
        assert!(gate.may_start());
    }

    #[test]
    fn apres_une_fin_une_trame_detat_ne_peut_pas_demarrer_un_match() {
        // C'est l'écran de fin de partie qui parle, pas un nouveau match.
        let mut gate = MatchGate::new();
        gate.closed();
        assert!(!gate.may_start());
    }

    #[test]
    fn apres_une_fin_puis_une_ouverture_une_trame_detat_peut_de_nouveau_demarrer_un_match() {
        // La partie suivante commence vraiment.
        let mut gate = MatchGate::new();
        gate.closed();
        gate.opened();
        assert!(gate.may_start());
    }

    #[test]
    fn deux_fins_consecutives_laissent_la_porte_fermee() {
        // `MatchEnded` puis `MatchDestroyed` : le jeu annonce la fin deux fois.
        let mut gate = MatchGate::new();
        gate.closed();
        gate.closed();
        assert!(!gate.may_start());
    }
}
