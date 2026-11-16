# kcd_bilingual_generator_rs 

一个简单的 Rust 双语生成器，适用于 Kingdom Come: Deliverance 1&2
这个bilingual generator按照中文的体验习惯生成双语文本，包括不显示某些过长的文本为双语，有些较短的文本用 '/' 做分隔符等。

---

## 使用教程  

### 1. 下载可执行文件  
前往 [Release 页面](https://github.com/325bit/kcd_bilingual_generator_rs/releases) 下载 `kcd_bilingual_generator_rust.exe`。  

### 2. 运行
运行 `kcd_bilingual_generator_rust.exe`。首次启动会在当前目录生成配置文件，对游戏路径和双语组合的修改都会保存到配置文件中。

### 3. 生成双语 Mod  
点击 **Generate Bilingual Pak** 按钮，生成 `.pak` 文件，这是双语 Mod 的核心文件。  

### 4. 准备 Mod  
1. 使用 [KCD Mod Generator](https://github.com/altire-dev/kcd-toolkit) 生成 Mod，或者：  
   - 随便下个[我的双语 Mod](https://mod.3dmgame.com/mod/217263)。  
   - 将其中的 `.pak` 文件替换为你生成的 `.pak` 文件。  
2. 修改 `mod.manifest` 文件：  
   - 更新 `modid`、`name`、`description`、`author` 和 `created_on` 字段。  
3. 根据你的双语配置重命名 Mod 文件夹。  

**示例结构**  
原结构：  
```
简中 + 英 v2.51  
 ┣ Localization  
 ┃ ┗ Chineses_xml.pak  
 ┗ mod.manifest  
```
重命名为：  
```
英语 + 日语 v2.51  
 ┣ Localization  
 ┃ ┗ English_xml.pak  
 ┗ mod.manifest  
```

#### 注意事项：  
- `XXX_xml.pak` 文件名中的 `XXX`（语言）决定替换游戏中的哪种语言。比如，改为 `English_xml.pak` 时，这个.pak文件会替换游戏的英文文本。  
- 如果出现口口，说明当前语言的字体不支持口口语言的完口显口。你应该去[Better Chinese Font mod](https://www.nexusmods.com/kingdomcomedeliverance2/mods/53)下个字体，而不是来找我报错，口口跟我没关系。 

### 5. 安装 Mod  
将重命名后的 Mod 文件夹放入游戏的 `Mods` 目录（位于游戏根目录下），启动游戏即可。  