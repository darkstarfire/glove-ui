//! A janela de senha de administrador para os apps que rodam um comando com o sudo.
//!
//! Quem pergunta a senha é o `glove-senha` (o mesmo programa do agente do polkit), numa
//! janela própria: `glove-senha --sudo`, com o que vai ser feito em GLOVE_SENHA_MENSAGEM.
//! Ele confere a senha com o próprio sudo, mostra o erro ali mesmo e só então a escreve
//! na saída. O app continua como antes: passa a senha ao `sudo -S` do comando dele.

use std::io::Read;
use std::process::{Child, Command, Stdio};

/// Um pedido de senha aberto (a janela do glove-senha).
pub struct Pedido {
    child: Child,
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

/// O glove-senha ao lado deste programa (a instalação põe todos juntos); senão, o do PATH.
fn programa() -> std::path::PathBuf {
    std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.join("glove-senha"))).filter(|p| p.exists()).unwrap_or_else(|| "glove-senha".into())
}

impl Pedido {
    /// Abre a janela de senha, dizendo o que vai ser feito ("Atualizar o Glove.").
    /// None se o glove-senha não está instalado: quem chama pergunta na própria janela.
    pub fn novo(o_que: &str) -> Option<Self> {
        Self::com(Command::new(programa()).arg("--sudo").env("GLOVE_SENHA_MENSAGEM", o_que))
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
        assert!(Pedido::com(&mut Command::new("glove-senha-que-nao-existe")).is_none());
        // Largar o pedido fecha a janela que ficou aberta (não espera os 30 s).
        let t = Instant::now();
        drop(Pedido::com(Command::new("sleep").arg("30")).unwrap());
        assert!(t.elapsed() < Duration::from_secs(5));
    }
}
