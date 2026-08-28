# The public record

Everything found in the open about the claim that a 2019 Bose firmware update degraded
active noise cancelling on the QuietComfort 35 and QuietComfort 35 II. Collected as
background for the binary analysis in `README.md`.

## Timeline

| Date | Event |
|---|---|
| Mar 2019 | QC35 II firmware **4.3.6** ships. Release notes: lays "the groundwork to support Bose AR". |
| Jun 18 2019 | QC35 II firmware **4.5.2** ships, alongside **3.0.3** for the first generation QC35. Release notes mention music sharing improvements. |
| Jul 2019 | Complaints begin on the Bose Community forum and on Reddit. The recurring description is that noise cancelling became weaker and that the low and high levels stopped sounding different. |
| 2019 | Bose blocks the ability to downgrade, citing "identified security concerns". A Change.org petition asking Bose to withdraw 4.5.2 and 3.0.3 gathers roughly 898 signatures. |
| Nov 2019 | The Register covers the complaints. Bose says a dedicated team is investigating. |
| Late 2019 | Bose offers to phone affected owners and, in some cases, to visit their homes to test the headphones in the owner's own environment. |
| Apr 2 2020 | Bose publishes the *QC35 II Firmware 4.5.2 Noise Cancellation Investigation Report*. Conclusion: the firmware did not affect noise cancelling. |
| Apr 2020 | Despite that conclusion, Bose re-opens downgrades for a limited period, to **4.3.6** on the QC35 II and **2.5.5** on the QC35. |
| Oct 7 2020 | QC35 II firmware **4.8.1** ships, adding "Self-Voice" and a wired-mode sleep timer. |

The Bose Community forum thread grew past 200 pages, reported at one point as 232 pages.
That forum has since been retired, so the thread is no longer reachable at its original
location.

## What Bose said

The core finding, as quoted in press coverage of the report: the degradation in all cases
was the result of hardware related issues with ear cushions, aftermarket parts, or
mechanical integrity. Bose stated that no direct or indirect changes were made to the
firmware's noise cancelling feature.

Bose's Lead Community Manager, posting as Wayne_M: "Through all of our investigation and
testing, we're confident that firmware 4.5.2 did not affect the noise cancelling feature."

On the accusation of deliberate degradation: "we would never intentionally downgrade the
performance of our products in the field."

On the downgrade: "Today, we're re-introducing the ability to downgrade firmware QC35 II to
4.3.6 and QC35 series 1 to 2.5.5 via the Bose BTU site for a limited time."

Bose reported that eight of the ten units it examined in depth were functioning correctly.
In several cases it attributed the perception of weak noise cancelling to third-party ear
cups, or to ear cups that had not been clicked fully into place.

## What users said

The suspicion, widely repeated, was commercial. The Noise Cancelling Headphones 700 had
recently launched, and owners suspected the older model was being degraded to push the
upgrade. One owner quoted by The Register: "They did this same thing when they updated the
QC35 to the QC35II. It is very clear that they are deliberately breaking the firmware when
they have a new product that comes out."

Another, on the practical effect: "Very disappointed with Bose as I only purchased these
headphones in April and they are basically useless to me now on my commute and in the noisy
open office."

Not everyone agreed. Some owners reported no change, and some reported that a factory reset
after updating resolved what they were hearing.

Two methodological objections to Bose's report came up repeatedly, and both are worth
carrying into the binary analysis.

1. **Bose compared the wrong pair.** The report compared 4.1.3 against 4.5.2. Almost all
   complaints came from owners upgrading from 2.x or 3.x, so the tested path was not the
   path most users took. Every one of those versions is archived in `firmware/baywolf/`,
   so this is now directly testable.
2. **Bose tested the wrong noise.** The listening tests centred on continuous aircraft
   noise. Several complaints specifically described impulsive sounds, keyboard typing being
   the common example. A regression confined to transient or burst noise would not show up
   in a steady-state test.

## Was there a lawsuit?

No class action over the noise cancelling firmware was found. There is a separate and
frequently confused matter: a class action concerning a QC35 **power switch** defect, where
the switch fails and the headphones will not turn off. That is a mechanical complaint and is
unrelated to firmware 4.5.2. Note, though, that the same switch fault came up in technical
discussion of the ANC complaints, since a switch that re-energises the headphones can drain
the battery overnight and produce degraded behaviour that a user would reasonably read as an
ANC problem.

There was also an earlier and unrelated 2017 privacy suit about the Bose Connect app
collecting listening data.

## Prior reverse engineering and community work

This is the answer to "has anyone already tried this". Several people have worked on Bose
firmware. Nobody found appears to have published a binary diff establishing whether the ANC
code or coefficients actually changed in 4.5.2, which is the gap this directory fills.

| Project | What it is | Relevance |
|---|---|---|
| [bosefirmware/ced](https://github.com/bosefirmware/ced) | The main community firmware archive. Holds the full historical set for many Bose products, including every QC35 and QC35 II release. Also documents the downgrade procedure. | The source of the 4.5.2 images used here. Bose has removed 4.5.2 from its own servers. |
| [tchebb/bose-dfu](https://github.com/tchebb/bose-dfu) | An open-source command line firmware tool in Rust, working over USB HID. Runs on Windows, macOS, and Linux, and unlike Bose's own updater it will downgrade. QC35 II is listed as partially supported. | The cleanest route to flashing a chosen version. |
| [avicoder/Bose-headphones-firmware](https://github.com/avicoder/Bose-headphones-firmware) | A smaller archive of QC35 II images, 2.5.1, 3.1.8, and 4.8.1. The README suggests binwalk. | Useful as an independent copy for cross-checking. Its 4.8.1 files match Bose's exactly. |
| [HarrytheOrange/boseReverse](https://github.com/HarrytheOrange/boseReverse) | States its goal as recovering the ANC on QC30 and QC35 and building an unofficial firmware. Holds 2.5.5 and 3.0.3 coefficient files. | Closest in intent to this analysis, but only three commits and no published findings. Effectively abandoned. |
| [sunzj/Way_of_Downgrade_BOSE_QC35ii](https://github.com/sunzj/Way_of_Downgrade_BOSE_QC35ii) | A downgrade method that races the updater, swapping the temporary files it writes before it flashes them. Author downgraded 4.5.2 to 2.5.1 and 3.1.8. | Also the source of the observed 4.5.2 file sizes, which corroborate the archived copies. The author warns the method risks damaging the device. |
| [MadEyez/quietrebellion](https://github.com/MadEyez/quietrebellion) | A reverse-engineered implementation of Bose's BMAP control protocol, for the QC Ultra. | Protocol-level rather than firmware-level, but relevant if runtime ANC state needs to be read back from a device. |
| Linux Bluetooth mailing list | Documents QC35 control over RFCOMM channel 8, using a three-byte opcode, a one-byte length, and a payload. | A route to querying and setting the noise cancelling level on a live device. |
| [paulodeleo/bose-qc35-1-firmwares](https://github.com/paulodeleo/bose-qc35-1-firmwares) | First generation QC35 backups, 2.5.5 and 3.0.3, plus both index files. | An independent copy of the gen-1 pair that was blamed. |

### Silicon and tooling

The QC35 II is built on a CSR, later Qualcomm, BlueCore part. That family pairs an XAP
application processor with a Kalimba DSP for audio. Useful background for step 1 of the
analysis plan:

- An open-source XAP assembler exists, originally from darkircop.org and revived since.
- Some CSR toolchain source has been published on GitHub, including GPL code CSR had
  omitted to release.
- An IDA plugin for XAP2 has been discussed on mailing lists over the years. Availability
  and quality were not confirmed.
- Kalimba is the DSP core in these parts. No mature public disassembler for it was found.

Practical note: the `.dfu` payload is neither encrypted nor compressed, measured at 6.35
bits of entropy per byte, so none of this requires defeating protection. It requires a
disassembler for the right architecture.

## How to downgrade, if you want to test by ear

Two routes. Both carry risk.

**Bose's own updater, with the hidden advanced menu.** The community-patched updater
binaries in `bosefirmware/ced` point the tool at an alternative index that still lists old
releases. With the patched updater installed, connect the headphones by USB, open
`https://btu.bose.com`, launch the app when prompted, then press `a`, `d`, `v`, up arrow,
down arrow to reveal the version selector.

**bose-dfu.** A standalone tool, no patched Bose software involved. `enter-dfu`, then
`download` the chosen image, then `leave-dfu`.

Whichever route, note the archived warning: **on a QC35 II whose serial number ends in
`AZ`, do not downgrade below 2.1.3. Doing so can brick the headphones.**

Worth trying first, since it costs nothing and several owners reported it helped: a factory
reset, and a careful check that the ear cushions are genuine and fully clicked into place.
That was, after all, Bose's own explanation, and the coefficient evidence in `README.md`
does not contradict it.

## Sources

- [The Register, Bose shouts down claims that it borked noise cancellation firmware](https://www.theregister.com/2020/04/06/bose_allows_downgrades_for_qc_anc_headphones_to_quiet_critics/)
- [The Register, Nov 2019 coverage](https://www.theregister.com/2019/11/26/bose_firmware_borks_headphones/)
- [TechRadar, Bose QC35 owners claim noise cancelling problem caused by software update](https://www.techradar.com/news/bose-qc35-noise-cancelling-was-gimped-by-recent-software-update-claim-owners)
- [Gizmodo, Bose Lets Users Downgrade QC35 Firmware After Months of Complaints](https://gizmodo.com/bose-lets-users-downgrade-qc35-firmware-after-months-of-1842706931)
- [Digital Trends, Bose denies firmware fried ANC on QC35 headphones but will allow downgrades](https://www.digitaltrends.com/home-theater/bose-qc35-firmware-anc-damage-rollback/)
- [What Hi-Fi, Bose offers home visits to investigate QC35 II noise-cancelling issue](https://www.whathifi.com/news/bose-offers-home-visits-to-investigate-quietcomfort-35-ii-headphones-issue)
- [What Hi-Fi, Users complain of poor sound quality after Bose headphones firmware update](https://www.whathifi.com/news/users-complain-poor-sound-quality-after-bose-headphones-firmware-update)
- [Gear Patrol, Have Your Bose QC35s Been Sounding Weird?](https://www.gearpatrol.com/tech/audio/a709124/bose-quietcomfort-firmware-downgrade/)
- [Beebom, Bose Now Lets You Downgrade Firmware of QC 35](https://beebom.com/bose-headphone-firmware-anc/)
- [Hacker News discussion of the Bose investigation report](https://news.ycombinator.com/item?id=22782814)
- [Change.org petition](https://www.change.org/p/bose-bose-change-and-take-down-the-4-5-2-and-3-0-3-firmware-for-qc35-and-qc35ii)
- [Bose Wikia, QuietComfort 35 II firmware history](https://bose.fandom.com/wiki/QuietComfort_35_wireless_headphones_II)
- [Linux Bluetooth list, QC35 battery and ANC control](https://www.spinics.net/lists/linux-bluetooth/msg88302.html)
- [ClassLawGroup, QC35 power switch failure lawsuit](https://www.classlawgroup.com/consumer-protection/bose-qc35-power-switch-failure-lawsuit) (separate matter, mechanical)
