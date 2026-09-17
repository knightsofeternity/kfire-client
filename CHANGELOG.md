# Changelog

Release notes for the KFIRE desktop client.

The section matching the tag being built becomes the body of the GitHub
Release, so every version MUST have its own section here before it is tagged.
The release build fails when one is missing, which is deliberate: publishing a
version under the previous one's notes has already happened once.

## 0.6.0-beta.2

**Still a beta of 0.6.0, for one tester.** Published as a pre-release, so the
update indicator in everybody else's client keeps pointing at 0.5.0.

**What this build fixes:** the first beta could fail without saying anything at
all. It tracked nothing, and neither the screen nor the log could tell you which
of four different things had gone wrong. No new tracking features here, only the
ability to see what is happening.

The Rocket League section of the settings now shows one line of plain status,
refreshed while you play:

- *Rocket League n'est pas lancé.* The tracker is on, but the game is not
  running.
- *En attente de la socket du jeu.* The tracker is watching and the game is
  running, but the game is not answering. Almost always because the game was
  already open when you enabled tracking: it only reads its configuration when
  it starts, so restart it. If another Rocket League tracker is running on this
  machine, it may also be holding the socket.
- *Connecté à Rocket League, mais le jeu n'envoie rien.* Connected, but nothing
  is coming through. Usually because you are sitting in a menu: the socket only
  speaks during a match.
- *Connecté à Rocket League, N messages reçus.* A number that climbs means the
  socket, the frame splitting and the decoding all work.

The log also names, once each, any event the game sends that this client does
not handle. If a game update ever renames the start or the end of a match, that
line is the only place it would show.

Your player name is never written to the log. The log records that one is set,
not what it is.

## 0.6.0-beta.1

**A beta of 0.6.0, for one tester.** This build is not offered to anyone else:
it is published as a pre-release, so the update indicator in everybody's client
keeps pointing at 0.5.0.

**New in this version:** KFIRE records your Rocket League matches. The game has
no player API, so the client reads the statistics socket the game opens on your
own machine, and sends nothing but a summary: the playlist, the result, both
team scores, and your own goals, assists, saves, shots, score and demolitions.
While you play, it also shows the live score on the guild portal, which is
broadcast and never stored anywhere.

The game must be told to open that socket, so the client writes one block into
a file in its install directory. It shows you the exact path and the exact
contents before you agree, and turning the tracking off takes the block back
out. **The game only reads that file when it starts**, so restart Rocket League
after enabling it.

Rocket League does not say which of the players in a match is you, so you have
to type your in-game name in the settings. That name never leaves your machine:
it only picks your row out of the scoresheet. If it matches nobody, the settings
screen shows you the names the game actually used, so you can copy the right one.

The other players in a match, team-mates and opponents alike, are named in what
the game sends. None of them ever leaves your machine, and the server has no
column that could hold a name either.

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
