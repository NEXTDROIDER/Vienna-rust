Vienna Modding API SDK
Version: 0.0.4

Contents:
- vma\                Rust crate with the Vienna Modding API
- example-hello-mcemod\  Example Rust mod that exports a .mcemod plugin

Typical flow:
1. Open the example mod.
2. Replace its logic with your own hooks.
3. Build it as a cdylib.
4. Rename or package the resulting DLL as .mcemod.
5. Put the .mcemod file into the server's mods folder.
