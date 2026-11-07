//! Junta os catálogos de tradução (.po) na compilação: os da própria glove-ui
//! (locales/ na raiz do repositório) e os do app que a usa, indicados pela
//! variável GLOVE_LOCALES (uma pasta com en.po, es.po…). O mesmo idioma nos dois
//! lugares é somado (o do app vale por cima).

use std::collections::BTreeMap;
use std::path::PathBuf;

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let mut pastas = vec![manifest.join("../../locales")];
    println!("cargo:rerun-if-env-changed=GLOVE_LOCALES");
    if let Ok(d) = std::env::var("GLOVE_LOCALES") {
        if !d.is_empty() {
            pastas.push(PathBuf::from(d));
        }
    }
    let mut por_idioma: BTreeMap<String, String> = BTreeMap::new();
    for pasta in &pastas {
        println!("cargo:rerun-if-changed={}", pasta.display());
        let Ok(rd) = std::fs::read_dir(pasta) else { continue };
        let mut arqs: Vec<PathBuf> = rd.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "po")).collect();
        arqs.sort();
        for p in arqs {
            println!("cargo:rerun-if-changed={}", p.display());
            let idioma = p.file_stem().unwrap().to_string_lossy().into_owned();
            let texto = std::fs::read_to_string(&p).unwrap_or_default();
            let t = por_idioma.entry(idioma).or_default();
            t.push_str(&texto);
            t.push('\n');
        }
    }
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let mut rs = String::from("/// Catálogos embutidos: (idioma, conteúdo do .po).\nconst EMBUTIDOS: &[(&str, &str)] = &[\n");
    for (idioma, texto) in &por_idioma {
        let arq = out.join(format!("{idioma}.po"));
        std::fs::write(&arq, texto).unwrap();
        rs.push_str(&format!("    ({idioma:?}, include_str!(concat!(env!(\"OUT_DIR\"), \"/{idioma}.po\"))),\n"));
    }
    rs.push_str("];\n");
    // Pastas varridas pelo teste que confere se todo tr!(…) tem tradução.
    let raizes: Vec<String> = pastas.iter().filter_map(|p| p.parent().map(|r| r.join("crates").display().to_string())).collect();
    rs.push_str(&format!("#[cfg(test)]\nconst PASTAS_DO_CODIGO: &[&str] = &{raizes:?};\n"));
    std::fs::write(out.join("catalogos.rs"), rs).unwrap();
}
