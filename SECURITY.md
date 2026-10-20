# Sicherheit

fw-ams enthält mit dem Hilfsdienst (`fw-ams-service`, läuft als LocalSystem) und dem Zugriff auf
Embedded Controller und SMU sicherheitsrelevante Teile. Meldungen nehmen wir ernst.

## Unterstützte Versionen

Sicherheitskorrekturen gibt es für die jeweils neueste Version aus den
[Releases](https://github.com/M-0-K/fw-ams/releases).

## Eine Lücke melden

Bitte **nicht** als öffentliches Issue, sondern vertraulich über GitHub:
[Security → Report a vulnerability](https://github.com/M-0-K/fw-ams/security/advisories/new).

Hilfreich sind: betroffene Version, Betriebssystem, Schritte zum Nachvollziehen und die
mögliche Auswirkung. Du bekommst in der Regel innerhalb einer Woche eine Rückmeldung. Nach der
Korrektur veröffentlichen wir einen Hinweis und nennen dich auf Wunsch als Finder.

## Sicherheitsmodell in Kürze

- Die App läuft ohne Adminrechte. Nur der Hilfsdienst hat erhöhte Rechte.
- Die Named Pipe `\\.\pipe\fw-ams-tuner` ist nur für SYSTEM, Administratoren und den
  installierenden Benutzer zugänglich und lehnt Verbindungen über das Netzwerk ab.
- Der Dienst akzeptiert nur typisierte Anfragen; jeder Wert wird auf den erlaubten Bereich
  geklemmt. Rohe SMU- oder Registerbefehle sind über die Pipe nicht möglich.
- Der Dienst läuft aus `%ProgramFiles%\fw-ams-tuner`, damit normale Benutzer die Datei nicht
  austauschen können.
- Das PawnIO-Modul `RyzenSMU.bin` ist vom PawnIO-Autor signiert und wird mit fester Version und
  SHA-256 heruntergeladen.

Besonders interessant sind daher: Umgehungen der Pipe-Zugriffsrechte, Werte außerhalb der
Grenzen, die der Dienst doch annimmt, und alles, was einem normalen Benutzer Codeausführung im
Dienst ermöglicht.
