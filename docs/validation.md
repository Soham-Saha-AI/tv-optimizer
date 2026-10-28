# Test build validation — 24 October 2026

This is a portable test build for owner validation, not a claim of completed hardware certification.

## Completed

- 58 native unit tests and 14 frontend tests passed. TypeScript and the production Windows build passed.
- Recovery tests cover exact enabled/default states, absent settings, immutable starting snapshots, exclusion of imported history from Restore all, reverse-order DNS changes, launcher dependencies, manual-change conflicts and interrupted operations.
- Earlier read-only inspection was performed on one Android TV.
- The native Windows interface ran with the development server stopped, using embedded assets and the bundled fixed WebView2 runtime. Normal Windows window controls are retained.
- The service-purpose notes and extra warning for uninstalling an already-disabled service were checked in the native interface without applying removal.
- Starting-point persistence and existing-configuration detection were checked on one Android TV; no optimization changes were applied during that check.
- Original CleanYourTV v2.7.0 was built and its APK inspected using an existing development toolchain. No launcher installation is part of this check.

## Still needs hardware or clean-machine validation

- The starting-point scan and database persistence were verified on the test TV. Restore all execution still needs a deliberate live-TV change/restore round trip, followed by the physical remote and playback checklist.
- Test guarded Home rollback after disconnecting the PC, withheld confirmation, TV reboot, DNS failure and restoration, and actual app disable/uninstall/restore sequences.
- Test the ZIP on a clean Windows x64 machine with no installed ADB or WebView2.
- Test the optional original-launcher toolchain download/build path on a machine with no existing Java or Android SDK. The existing-toolchain path passed.
- Other hardware remains unvalidated. This package has no webOS control backend.

Private app data, sign-ins, deleted cache and older launcher versions do not have a guaranteed full recovery path. Restore all lists these limits and refuses conflicting recorded states before execution where possible; each individual restore rechecks again.

The shipped ZIP starts with an empty data folder. Device snapshots, history and trust keys from development must never be copied into it. Keep the user's portable data folder with their application after first use.
