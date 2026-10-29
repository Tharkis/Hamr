## Install

| System | Download | How |
| --- | --- | --- |
| Windows 10/11 | `Hamr-*-windows-x64-setup.exe` | Run the installer. Windows may warn that the publisher is unknown, because the installer isn't code-signed yet: choose "More info", then "Run anyway". A portable `.zip` is also attached. |
| Arch Linux, CachyOS | `hamr-*-x86_64.pkg.tar.zst` | `sudo pacman -U hamr-*.pkg.tar.zst` |
| Debian, Ubuntu, Mint | `hamr_*_amd64.deb` | `sudo apt install ./hamr_*_amd64.deb` |
| Fedora, openSUSE | `hamr-*.x86_64.rpm` | `sudo dnf install ./hamr-*.x86_64.rpm` |
| Any distro, sandboxed | `Hamr-*-x86_64.flatpak` | `flatpak install --user Hamr-*-x86_64.flatpak` (needs the Flathub remote for the runtime) |
| Any distro, no install | `Hamr-*-x86_64.AppImage` | `chmod +x Hamr-*.AppImage` and run it |

Hamr needs a GPU driver with Vulkan support (Mesa on Linux). `SHA256SUMS.txt` lists checksums for every file.
