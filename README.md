# kcd_bilingual_generator_rs 

A simple Rust-based bilingual generator for *Kingdom Come: Deliverance 1 & 2  
This bilingual generator follows Chinese user experience conventions when producing bilingual text, including:  
- Automatically hiding excessively long texts from being displayed bilingually  
- Using '/' as a separator for shorter text segments  

As is well known, Chinese characters typically convey more information density at the same visual length compared to alphabetic languages. If you wish to modify the bilingual display logic, you're welcome to edit the `bilingual_generator.rs` file on your own.
中文介绍看[这里](README_zh.md)

---

## Usage Guide  

### 1. Download the Executable  
Go to the [Releases section](https://github.com/325bit/kcd_bilingual_generator_rs/releases) and download the latest version of `kcd_bilingual_generator_rust.exe`.  

### 2. Set Up Files  
1. Create a folder named `assets` in the same directory as the executable.  
2. Inside the `assets` folder, create a file named `bilingual_set.txt`.  
   - For formatting examples, refer to [this sample file](https://github.com/325bit/kcd_bilingual_generator_rs/blob/main/assets/bilingual_set.txt) in the GitHub repository.  

### 3. Generate the Bilingual Mod  
Run the downloaded `.exe` and click the **Generate Bilingual Pak** button. This will automatically create a `.pak` file, which is the core of the bilingual mod.  

### 4. Prepare the Mod  
1. Use the [KCD Mod Generator](https://github.com/altire-dev/kcd-toolkit) to create a mod, or:  
   - Download any of my existing bilingual mods.  
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
- The `XXX_xml.pak` filename (`XXX` = language) determines which in-game language the subtitles replace.  
  - Example: Renaming to `English_xml.pak` will switch subtitles to English.  
- If you see **square characters (口口)**, the current font does not fully support the target language.  
  - **Fix this yourself**: Go to [Better Chinese Font  mod](https://www.nexusmods.com/kingdomcomedeliverance2/mods/53) to create/download a custom font mod. Do not report this as an issue here.  

### 5. Install the Mod  
Move your renamed mod folder into the `Mods` directory in your KC:D game root folder and Launch the game.