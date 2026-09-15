# Changelog

Release notes for the KFIRE desktop client.

The section matching the tag being built becomes the body of the GitHub
Release, so every version MUST have its own section here before it is tagged.
The release build fails when one is missing, which is deliberate: publishing a
version under the previous one's notes has already happened once.

## 0.5.0

**New in this version:** KFIRE now records your Hearthstone matches. The game
has no player API, so the client reads the game's own logs on your machine and
sends nothing but a summary: the mode, the result, the number of turns, your
finishing place and the hero you played.

Your opponent's name and the cards played NEVER leave your machine. They are in
the log, and the client deliberately does not carry them; the server has no
column that could hold them either.

Tracking is off until you turn it on. Hearthstone only writes detailed logs when
a configuration file exists, and KFIRE shows you the exact path and the exact
contents before writing it. Turning tracking off removes the file again.

Battlegrounds only reports a finishing place, never a rating: the rating appears
nowhere in the game's logs, so KFIRE measures standing by average placement and
top-four rate instead.

Windows and macOS only, Hearthstone having no native Linux client.

## 0.4.0

**New in this version:** KFIRE detects a lot more games. Titles whose binary has
a generic name (`game.exe`, `hl2.exe` and friends) used to be invisible because
that name could belong to any of hundreds of games; KFIRE now recognizes them by
their install folder as well. That covers around 470 games, among them
Counter-Strike: Source, Day of Defeat: Source, Alien: Isolation, Football
Manager, Total War: Warhammer and DRAGON BALL GEKISHIN SQUADRA. Install paths
are read and compared on your machine only: KFIRE still reports nothing but the
game itself.
