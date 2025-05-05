#[cfg(target_os = "windows")]
extern crate winres;

// 辅助函数：将 "major.minor.patch" 格式的版本字符串转换为 Windows 需要的 u64 二进制格式
#[cfg(target_os = "windows")]
fn version_string_to_u64(version_str: &str) -> Result<u64, Box<dyn std::error::Error>> {
    let parts: Vec<&str> = version_str.split('.').collect();
    if parts.len() < 2 || parts.len() > 3 {
        // 允许 "major.minor" 或 "major.minor.patch"
        return Err(format!("Invalid version string format: {}", version_str).into());
    }

    let major = parts[0].parse::<u16>()?;
    let minor = parts[1].parse::<u16>()?;
    let patch = if parts.len() > 2 {
        parts[2].parse::<u16>()?
    } else {
        0
    };
    let build: u16 = 0; // 通常将 Cargo.toml 中没有的 build 号设为 0

    // 组合成 u64: (Major << 48) | (Minor << 32) | (Patch << 16) | Build
    // 注意：Windows VS_VERSION_INFO 结构实际是两个 32 位数 (dwFileVersionMS, dwFileVersionLS)
    // dwFileVersionMS = (Major << 16) | Minor
    // dwFileVersionLS = (Patch << 16) | Build
    // winres 期望的 u64 似乎是直接组合，文档可能不清晰，但通常做法是按位组合
    // 修正：根据常见做法和 rc.exe 行为，字符串形式更可靠，二进制形式有时可以简化
    // 为了符合 winres 的 u64 预期，我们假设它需要平面组合 (虽然这可能不是技术上最精确的表示)
    // 我们优先设置准确的字符串版本，二进制版本可以按需设置。
    // 一个更稳妥的二进制做法是只设置主要和次要版本，或者干脆不设置二进制让 RC 编译器处理。
    // 但如果需要设置，可以尝试这样：
    let version_u64 =
        ((major as u64) << 48) | ((minor as u64) << 32) | ((patch as u64) << 16) | (build as u64);

    // **重要更新/简化:** 查阅 winres 用法和 Windows 资源，最可靠的方式是:
    // 1. 主要设置字符串版本 (`FileVersion`, `ProductVersion`)。
    // 2. 如果需要设置二进制版本，可以尝试解析或用固定值。
    // 鉴于 u64 组合的歧义和复杂性，我们可以简化，优先保证字符串正确。

    // 这里我们还是尝试提供一个解析函数，但需谨慎使用
    Ok(version_u64)
}

// 辅助函数创建四部分的版本字符串，例如 "0.6.114.0"
#[cfg(target_os = "windows")]
fn create_file_version_string(version_str: &str) -> String {
    let parts: Vec<&str> = version_str.split('.').collect();
    let major = parts.get(0).cloned().unwrap_or("0");
    let minor = parts.get(1).cloned().unwrap_or("0");
    let patch = parts.get(2).cloned().unwrap_or("0");
    let build = "0"; // 添加 build 号 0
    format!("{}.{}.{}.{}", major, minor, patch, build)
}

#[cfg(target_os = "windows")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::var("TARGET")?.contains("windows") {
        let mut res = winres::WindowsResource::new();
        res.set_icon("src/assets/icon.ico"); // 确保 icon.ico 在项目根目录

        // --- 设置版本信息 ---
        let pkg_version = env!("CARGO_PKG_VERSION"); // 获取 Cargo.toml 中的版本, e.g., "0.6.114"

        // 1. 设置字符串版本 (推荐，最重要)
        // ProductVersion 通常是三段 (Major.Minor.Patch)
        res.set("ProductVersion", pkg_version);
        // FileVersion 通常是四段 (Major.Minor.Patch.Build)，我们添加 .0
        res.set("FileVersion", &create_file_version_string(pkg_version));

        // 2. 设置二进制版本 (可选，如果需要精确控制)
        // 注意：解析和组合 u64 可能比较棘手，如果字符串版本已设置，有时可以省略二进制设置
        // 或者使用简化/固定值。如果需要精确，使用上面的解析函数。
        match version_string_to_u64(pkg_version) {
            Ok(binary_version) => {
                // 设置相同的二进制版本给 File 和 Product
                res.set_version_info(winres::VersionInfo::FILEVERSION, binary_version);
                res.set_version_info(winres::VersionInfo::PRODUCTVERSION, binary_version);
            }
            Err(e) => {
                eprintln!("cargo:warning=Failed to parse package version '{}' for binary version info: {}. Skipping binary version.", pkg_version, e);
                // 如果解析失败，可以选择不设置二进制版本，或设置一个默认值
                // res.set_version_info(winres::VersionInfo::FILEVERSION, 0x0000000000000000); // Example: 0.0.0.0
                // res.set_version_info(winres::VersionInfo::PRODUCTVERSION, 0x0000000000000000);
            }
        }

        // --- 设置其他元数据 ---
        if let Some(description) = option_env!("CARGO_PKG_DESCRIPTION") {
            res.set("FileDescription", description);
        }
        // 使用 CARGO_PKG_NAME 设置产品名和原始文件名
        let pkg_name = env!("CARGO_PKG_NAME");
        res.set("ProductName", pkg_name);
        res.set("InternalName", &format!("{}.exe", pkg_name));
        res.set("OriginalFilename", &format!("{}.exe", pkg_name));

        // 设置公司名和版权信息
        // 建议从 Cargo.toml 的 authors 或 [package.metadata] 读取，如果固定也可以直接写
        let authors = env!("CARGO_PKG_AUTHORS"); // e.g., "Your Name <your.email@example.com>"
                                                 // 简单的处理：取第一个作者的名字部分作为公司名（或直接硬编码）
                                                 // let company_name = authors.split('<').next().unwrap_or("").trim();
        let company_name = format!(
            "Anon Tokyo - {}",
            authors.split('<').next().unwrap_or("").trim()
        );
        res.set(
            "CompanyName",
            if company_name.is_empty() {
                "Unknown"
            } else {
                &company_name
            },
        );

        // 版权信息，可以组合年份和作者/公司
        let current_year = 2025; // 你可以用 chrono crate 获取当前年份: chrono::Utc::now().year()
        res.set(
            "LegalCopyright",
            &format!("Copyright (C) {} {}", current_year, company_name),
        );

        // 编译资源文件
        if let Err(e) = res.compile() {
            // 使用 eprintln! 将错误输出到 stderr，cargo 会显示为错误
            eprintln!("Failed to compile Windows resource: {}", e);
            // 发生错误时退出构建脚本
            std::process::exit(1);
        }
    }
    Ok(())
}

// 如果目标不是 Windows，提供一个空的 main 函数
#[cfg(not(target_os = "windows"))]
fn main() {
    // 不需要做任何事
}
