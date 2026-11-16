# kcd_bilingual_generator_rs 

A simple Rust-based bilingual generator for Kingdom Come: Deliverance 1&2
This bilingual generator produces bilingual text following Chinese users' experience conventions, including: not displaying some excessively long texts bilingually, using '/' as a separator for some shorter text segments, and so on.
中文介绍看[这里](README_zh.md)

---

## Usage Guide  

### 1. Download the Executable  
Go to the [Releases section](https://github.com/325bit/kcd_bilingual_generator_rs/releases) and download `kcd_bilingual_generator_rust.exe`.  

### 2. Run It
Run `kcd_bilingual_generator_rust.exe`. On first launch it creates a config file in the current directory; any changes to the game path and bilingual pairs are saved to this config file.

### 3. Generate the Bilingual Mod  
Click the **Generate Bilingual Pak** button to generate a `.pak` file, which is the core of the bilingual mod.  

### 4. Prepare the Mod  
1. Use the [KCD Mod Generator](https://github.com/altire-dev/kcd-toolkit) to create a mod, or:  
   - Simply download [one of my bilingual mods](https://mod.3dmgame.com/mod/217263).  
   - Replace its `.pak` file with the one you generated.  
2. Edit the `mod.manifest` file:  
   - Update the `modid`, `name`, `description`, `author`, and `created_on` fields.  
3. Rename the mod folder based on your bilingual configuration.  

**Example Structure**  
Original:  
```
Chineses + English v2.51  
 ┣ Localization  
 ┃ ┗ Chineses_xml.pak  
 ┗ mod.manifest  
```
Renamed to:  
```
English + Japanese v2.51  
 ┣ Localization  
 ┃ ┗ English_xml.pak  
 ┗ mod.manifest  
```

#### Key Notes:  
- The `XXX` (language) in the `XXX_xml.pak` filename determines which in-game language is replaced. For example, renaming it to `English_xml.pak` makes the pak replace the game's English text.  
- If you see square characters (口口), the current font does not fully support that language's characters. You should download a font from the [Better Chinese Font mod](https://www.nexusmods.com/kingdomcomedeliverance2/mods/53) instead of reporting an issue to me — it has nothing to do with me. 

### 5. Install the Mod  
Move the renamed mod folder into the game's `Mods` directory (in the game root folder) and launch the game.  
