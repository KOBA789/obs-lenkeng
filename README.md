# obs-lenkeng

An OBS Studio Plugin for using LKV373 as a HDMI grabber

## WARNING

This is an experimental product.
Do NOT use this in critical scenes.

## How to build

obs-lenkeng includes git submodules.
`git submodule init` and `git submodule update` are need before you build it.

OBS Studio and libjpeg-turbo which obs-lenkeng depends on may require additional tools or libraries to build.
See the respective documentations for more information.

- [Install Instructions · obsproject/obs-studio Wiki](https://github.com/obsproject/obs-studio/wiki/Install-Instructions)
- [libjpeg-turbo/BUILDING.md at master · libjpeg-turbo/libjpeg-turbo](https://github.com/libjpeg-turbo/libjpeg-turbo/blob/master/BUILDING.md)

> NOTE:
> Only macOS is supported currently.
> If you succeed to build in other platforms, please let me know how to do.

And then, just run `cargo build`.

## TODO

- `CMAKE_ASM_NASM_COMPILER` is hard-corded in `libturbojpeg-sys/build.rs`
  - This is breaking the portability
- source_info.update is not implemented
  - To apply changes of the interface address, relaunch is need
- No Audio
- Lack of inter-frame jitter normalization
  - FPS relies on when the packet comes only
- De-interlace
