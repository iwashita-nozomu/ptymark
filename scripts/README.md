# Ptymark scripts

This directory contains product-owned installation, renderer validation, local package smoke, and release-metadata utilities.

- `installer.sh`, `installer.ps1`, `installer.cmd`: canonical source-install frontends;
- `install-managed-bundle.*`: isolated managed-renderer installation;
- `check-ptymark-renderers.sh`: selected renderer acceptance;
- `check-ptymark-runtime-dependencies.mjs`: version and dependency alignment;
- `package-release.*`: developer/CI-only local package smoke; outputs are discarded;
- `check-release-metadata.py`: source-only release contract validation.

Scripts must not edit shell profiles automatically, create a global Node installation, or publish executable release assets.
