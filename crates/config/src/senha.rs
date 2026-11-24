//! A janela de senha de administrador para os apps que rodam um comando com o sudo.
//!
//! Quem pergunta a senha é o `glove-auth` (o mesmo programa do agente do polkit), numa
//! janela própria: `glove-auth --sudo`, com o que vai ser feito em GLOVE_AUTH_MESSAGE.
//! Ele confere a senha com o próprio sudo, mostra o erro ali mesmo e só então a escreve
//! na saída. O app continua como antes: passa a senha ao `sudo -S` do comando dele.

use std::io::Read;
use std::process::{Child, Command, Stdio};

/// Um pedido de senha aberto (a janela do glove-auth).
pub struct Pedido {
    child: Child,
}

/// Um formulário de senha na janela do glove-auth (`glove-auth --fields`): a senha de
/// uma rede, trocar a senha da conta ou do chaveiro. A janela não confere nada; os
/// valores voltam para quem pediu.
pub struct Formulario<'a> {
    pub titulo: &'a str,
    pub mensagem: &'a str,
    /// (rótulo, esconde o que é digitado).
    pub campos: &'a [(&'a str, bool)],
    pub botao: &'a str,
    /// Nome do ícone (vazio = o cadeado).
    pub icone: &'a str,
    /// Os dois campos que têm de ser iguais (a senha nova e a confirmação).
    pub confirmar: Option<(usize, usize)>,
    /// Um aviso em vermelho ao abrir (a tentativa anterior não deu certo).
    pub erro: Option<&'a str>,
}

/// O que um formulário respondeu.
#[derive(Debug, PartialEq)]
pub enum Valores {
    Esperando,
    Cancelado,
    /// Os valores dos campos, na ordem.
    Prontos(Vec<String>),
}

#[derive(Debug, PartialEq)]
pub enum Resposta {
    /// A janela ainda está aberta.
    Esperando,
    /// Fechada sem a senha (Cancelar, Esc, ou a janela não abriu).
    Cancelado,
    /// A senha, já conferida.
    Senha(String),
}

/// O glove-auth ao lado deste programa (a instalação põe todos juntos); senão, o do PATH.
fn programa() -> std::path::PathBuf {
    std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.join("glove-auth"))).filter(|p| p.exists()).unwrap_or_else(|| "glove-auth".into())
}

impl Pedido {
    /// Abre a janela de senha, dizendo o que vai ser feito ("Atualizar o Glove.").
    /// None se o glove-auth não está instalado: quem chama pergunta na própria janela.
    pub fn novo(o_que: &str) -> Option<Self> {
        Self::com(Command::new(programa()).arg("--sudo").env("GLOVE_AUTH_MESSAGE", o_que))
    }

    /// Abre o formulário. None se o glove-auth não está instalado.
    pub fn formulario(f: &Formulario) -> Option<Self> {
        let campos: Vec<String> = f.campos.iter().map(|(l, secret)| if *secret { format!("*{l}") } else { l.to_string() }).collect();
        let mut cmd = Command::new(programa());
        cmd.arg("--fields")
            .env("GLOVE_AUTH_TITLE", f.titulo)
            .env("GLOVE_AUTH_MESSAGE", f.mensagem)
            .env("GLOVE_AUTH_FIELDS", campos.join("\n"))
            .env("GLOVE_AUTH_BUTTON", f.botao)
            .env("GLOVE_AUTH_ICON", f.icone)
            .env("GLOVE_AUTH_ERROR", f.erro.unwrap_or(""));
        if let Some((a, b)) = f.confirmar {
            cmd.env("GLOVE_AUTH_CONFIRM", format!("{a}-{b}"));
        }
        Self::com(&mut cmd)
    }

    /// Como está o formulário, sem esperar (para chamar a cada quadro).
    pub fn valores(&mut self) -> Valores {
        match self.child.try_wait() {
            Ok(None) => Valores::Esperando,
            Ok(Some(status)) if status.success() => {
                let mut out = String::new();
                if let Some(mut o) = self.child.stdout.take() {
                    let _ = o.read_to_string(&mut out);
                }
                Valores::Prontos(out.lines().map(str::to_string).collect())
            }
            _ => Valores::Cancelado,
        }
    }

    fn com(cmd: &mut Command) -> Option<Self> {
        let child = cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().ok()?;
        Some(Self { child })
    }

    /// Como está o pedido, sem esperar (para chamar a cada quadro).
    pub fn resposta(&mut self) -> Resposta {
        match self.child.try_wait() {
            Ok(None) => Resposta::Esperando,
            Ok(Some(status)) if status.success() => {
                let mut senha = String::new();
                if let Some(mut out) = self.child.stdout.take() {
                    let _ = out.read_to_string(&mut senha);
                }
                let senha = senha.trim_end_matches(['\n', '\r']).to_string();
                if senha.is_empty() { Resposta::Cancelado } else { Resposta::Senha(senha) }
            }
            _ => Resposta::Cancelado,
        }
    }
}

impl Drop for Pedido {
    /// O app fechou (ou desistiu) com a janela de senha aberta: ela fecha junto.
    fn drop(&mut self) {
        if let Ok(None) = self.child.try_wait() {
            let _ = self.child.kill();
        }
        let _ = self.child.wait();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn espera(p: &mut Pedido) -> Resposta {
        let t = Instant::now();
        loop {
            match p.resposta() {
                Resposta::Esperando if t.elapsed() < Duration::from_secs(5) => std::thread::sleep(Duration::from_millis(10)),
                r => return r,
            }
        }
    }

    #[test]
    fn valores_do_formulario() {
        let mut p = Pedido::com(Command::new("sh").args(["-c", "printf 'fabio\\nsegredo\\n'"])).unwrap();
        let t = Instant::now();
        let v = loop {
            match p.valores() {
                Valores::Esperando if t.elapsed() < Duration::from_secs(5) => std::thread::sleep(Duration::from_millis(10)),
                v => break v,
            }
        };
        assert_eq!(v, Valores::Prontos(vec!["fabio".into(), "segredo".into()]));
        let mut p = Pedido::com(Command::new("sh").args(["-c", "exit 1"])).unwrap();
        std::thread::sleep(Duration::from_millis(200));
        assert_eq!(p.valores(), Valores::Cancelado);
    }

    #[test]
    fn senha_cancelado_e_fechado() {
        // A janela devolveu a senha (sai com sucesso e a escreve).
        let mut p = Pedido::com(Command::new("sh").args(["-c", "sleep 0.1; echo 'minha senha'"])).unwrap();
        assert_eq!(p.resposta(), Resposta::Esperando);
        assert_eq!(espera(&mut p), Resposta::Senha("minha senha".into()));
        // Cancelar: sai com erro.
        let mut p = Pedido::com(Command::new("sh").args(["-c", "exit 1"])).unwrap();
        assert_eq!(espera(&mut p), Resposta::Cancelado);
        // Saiu com sucesso sem escrever nada: não é senha.
        let mut p = Pedido::com(&mut Command::new("true")).unwrap();
        assert_eq!(espera(&mut p), Resposta::Cancelado);
        // O programa não existe: sem janela.
        assert!(Pedido::com(&mut Command::new("glove-auth-que-nao-existe")).is_none());
        // Largar o pedido fecha a janela que ficou aberta (não espera os 30 s).
        let t = Instant::now();
        drop(Pedido::com(Command::new("sleep").arg("30")).unwrap());
        assert!(t.elapsed() < Duration::from_secs(5));
    }
}
