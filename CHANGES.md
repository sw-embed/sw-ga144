# Changes

Project changes, newest first, grouped by day. Times are Pacific
(UTC-08:00), as committed. Short Git SHAs identify the commits.
Changelog-only commits are omitted.

Categories include `feat` for new capabilities, `fix` for bug fixes,
`build` for build and deployment, `docs` for documentation, and
`chore` for repository and AgentRail bookkeeping.

## 2026-11-21

- 10:36 `chore` `3550db1` Recorded completion of the Double 21 fix and its regression checks in AgentRail.
- 10:36 `fix` `ddae270` Double 21 now holds 42 in node 000's T register as the chip keeps ticking. Added checks for 800 ticks of stable output and normal 18-bit addition wraparound.
- 10:14 `docs` `1670003` Added a disclaimer to the README and browser Help explaining that the demo is based on public documentation and has not been validated against GreenArrays hardware or software.
- 10:12 `docs` `98cc395` Added the SVFIG Forth Day origin story, a GreenArrays website link, and a screenshot of the live Double and forward demo to the README.
- 10:03 `docs` `238ea4b` Added the Arduino UNO Q, GPU emulator, and FPGA simulator to the roadmap. Recorded the verified GitHub Pages deployment in AgentRail.
- 10:01 `build` `a664130` Added the GitHub Actions workflow that builds the pinned X_eTaL runtime and deploys GA144 Studio to Pages. Linked the live demo and documented setup in the README.
- 09:58 `chore` `4cf8e11` Recorded completion of the F18 assembler and single-chip emulator step in AgentRail.
- 09:58 `feat` `945c924` Added the X_eTaL F18A assembler, Rust GA144 emulator with blocking port transfers, and Yew/WASM browser studio with four demos and a node inspector. Added the CLI, end-to-end tests, build instructions, and MIT license. Linked the official GreenArrays manuals and ignored local PDF copies.
- 09:53 `chore` `86a444f` Initialized the repository with a placeholder.
