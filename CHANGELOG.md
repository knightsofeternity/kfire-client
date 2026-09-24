# Changelog

Release notes for the KFIRE desktop client.

The section matching the tag being built becomes the body of the GitHub
Release, so every version MUST have its own section here before it is tagged.
The release build fails when one is missing, which is deliberate: publishing a
version under the previous one's notes has already happened once.

## 0.7.0

### A window that fits without scrolling

The client window no longer stacks everything in one long page. A column of
icons on the left gives each part its own tab: Home, one tab per tracked game
(Hearthstone, Rocket League, League of Legends) and Settings.

Each game icon carries a small dot: green when its tracking is on, orange when
something is missing for it to work (a folder not found, an in-game name not
filled in, a last match that matched nobody), grey when it is off. You see at a
glance which game needs attention.

A game's tab shows a single switch to turn its tracking on or off, the list of
what is sent to the server, what never leaves your computer, and your last
recorded match. The exact file KFIRE writes and its content are still there,
folded under "Technical details".

The window remembers the last tab you opened, and it now speaks French or
English, following your system's language.

## 0.6.2

### The client no longer forgets its server

Some members found their client back on the first screen, with an empty
server address, as if it had never been set up, and the games they played in
the meantime were not counted.

The client renews its session with the server every time it reconnects. Until
now, any error in that exchange was read as "this device's session is over",
and the client dropped the link along with the server address. But a server
that is restarting, for example during an update, briefly answers with an
error too. A client that reconnected at that exact moment unlinked itself.

Only a real refusal ends the link now: a session the server no longer knows,
or a banned account. Any other error, like a server that is restarting or busy,
or a page served by a proxy in front of it, is retried until the server
answers again, like a network outage.

When a session does end for real, the link form now opens with the server
address already filled in and says the session expired, so linking again
takes one click.

## 0.6.1

Two corrections on Hearthstone, both reported by a beta tester and both visible
on the portal.

### The number of turns was doubled

A Battlegrounds game that stopped at turn 18 was recorded, and shown live, as
36 turns. The client was reading the counter Hearthstone keeps on the game
itself, which in Battlegrounds ticks once for the recruit phase and once for
the combat: exactly twice the turn you see on screen. It now reads the counter
carried by your own player, which is the turn you played.

Matches already recorded keep the doubled number they were sent with: a client
cannot reach back into a server's history. An admin who wants the past corrected
can halve the turn count of past Battlegrounds rows, which is exact rather than
approximate since the old value was always exactly twice the real one.

If Hearthstone never writes that counter, a match is now sent with no turn count
at all rather than with a doubled one.

### Winning a Battlegrounds lobby was recorded as a second place

A top 1 came out as a top 2. Hearthstone only writes your leaderboard position
when an opponent dies, and never writes a 1 for the last player standing, so
the last position the log held was 2. A won Battlegrounds lobby is a first
place by definition, so a win is now recorded as first, live and at the end of
the match. Any other finish keeps the position the game gives. Battlegrounds wins
already stored keep their wrong position until an admin corrects them.

## 0.6.0

Coming from 0.5.0, which added Hearthstone match recording, this version brings
two things: Rocket League, and a live view of what the guild is playing right
now.

### Rocket League matches

KFIRE records your Rocket League matches. The game has no player API, so the
client reads the statistics socket the game opens on your own machine, and sends
nothing but a summary: the result, both team scores, and your own goals,
assists, saves, shots, score and demolitions.

The game only opens that socket if it is told to, so the client writes one block
into a file in the install directory. It shows you the exact path and the exact
contents before you agree, and turning tracking off takes the block back out.
**The game only reads that file when it starts**, so restart Rocket League after
enabling it.

Rocket League does not say which player in a match is you, so you type your
in-game name in the settings. That name never leaves your machine: it only picks
your row out of the scoresheet. If it matches nobody, the settings screen lists
the names the game actually used, so you can copy the right one.

### Your game in progress, on the portal

The portal has a **Live games** page showing what the guild is playing right
now, and this version feeds it for three games.

**Rocket League**: both scores, the clock, and your own statistics, refreshed
twice a second.

**Hearthstone**: the mode, the current turn, and your placement in Battlegrounds.
It refreshes every five seconds, which is how often the game's log is re-read.

**League of Legends**: your champion, level, KDA, creep score, gold and game
time, read from the API the game opens on your own computer while you play.
**Nothing to configure** for this one: the game says which player you are, so
there is no name to type. Turn it on in the settings.

None of this is ever stored. It is broadcast while you play and forgotten.

### What never leaves your machine

The same rule as Hearthstone in 0.5.0, now across four games.

Rocket League's scoresheet names every team-mate and opponent. League's API names
all ten players, with their teams and items. Hearthstone's log carries your
opponent's name and every card played. **None of it leaves your computer**, not
even to be displayed, and the server has no column able to hold a name.

Your own name does not leave either. It is used locally, and only to find your
own row.

Each game carries a test that pins the exact list of fields allowed out, so that
list cannot grow by accident.

Rocket League and Hearthstone are Windows and macOS only, neither having a
native Linux client.

## 0.6.0-beta.5

**Still a beta of 0.6.0.** Published as a pre-release, so the update indicator
in everybody else's client keeps pointing at 0.5.0.

### Rocket League matches that were never played

More than half the matches this client reported had never happened. The game
signals the end of a match TWICE, as `MatchEnded` and again as
`MatchDestroyed`, and it keeps sending state frames in between for the
end-of-match screen. This client built a match out of those frames, with the
same statistics as the real one and a clock restarted from zero, which is why
they all lasted well under a minute.

Of the fifteen matches stored on the server, eight were such ghosts. Only two
were duplicates of a real match; the other six arrived alone, because the real
report never made it, so nothing could have flagged them. They went straight
into your averages. Those rows have been removed.

A match can now only begin on an opening event, never on a state frame.

### The portal was not told a match had ended

The end of a live match was never announced: the server only found out when its
own timer expired the state, so a frozen score sat on the guild's live page for
up to fifteen seconds after the final whistle. It is announced now.

### Hearthstone on the live page

Your game in progress now appears on the portal's live page: the mode, the
current turn, and your placement in Battlegrounds.

Nothing else leaves this machine. The Hearthstone log carries your BattleTag,
your opponent's name and every card played; the portal receives three numbers
and the mode, and a test fails if that list ever grows.

The card refreshes every five seconds, which is how often the log is re-read.

### League of Legends on the live page

While you are in a game, League exposes an API on this computer. KFIRE reads it
and shows your champion, level, KDA, creep score, gold and game time on the
portal's live page.

**Nothing to configure**: the game itself says which player you are, so unlike
Rocket League there is no name to type and none to get wrong. Turn it on in the
settings.

That API names all ten players in the game, with their teams and items. None of
that leaves this machine; your own name does not either, it is only used here to
find your row among the ten.

Outside a game the API does not exist, and that is the normal state, not an
error.

## 0.6.0-beta.3

**Still a beta of 0.6.0, for one tester.** Published as a pre-release, so the
update indicator in everybody else's client keeps pointing at 0.5.0.

**What this build fixes:** no Rocket League match was ever recorded, and the
settings screen blamed your player name for it.

The client expected the game to say which playlist a match was played in. The
game never does: that field does not exist in Rocket League's statistics
protocol. Read as missing, it came out as the identifier of free play, so every
single match was refused as training before your name was even looked at. Then
the settings screen showed the "your name matched nobody" warning for any
refusal, which is why it could list your own name while claiming not to find it.

Training is now told apart from a real match with something the game actually
sends: a match has at least one player on the other team. The name warning only
appears when your name is really the problem; any other refusal is written to
the log with its actual reason.

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
