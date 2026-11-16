//! Comandos de fora com limite de tempo.
//!
//! Vários comandos de estado esperam para sempre um serviço que não está rodando: o
//! `bluetoothctl show` sem o bluetoothd, por exemplo, nunca termina. Quem chama um desses
//! da linha principal (o compositor, um app) travaria junto, e no compositor isso é a
//! tela inteira parada. Aqui o comando é encerrado quando passa do limite.

use std::{
    io::Read,
    process::{Command, Output, Stdio},
    time::{Duration, Instant},
};

/// Roda o comando e devolve a saída dele. None se o comando não existe ou se passou
/// de `limit` (aí ele é encerrado à força).
pub fn output_within(cmd: &mut Command, limit: Duration) -> Option<Output> {
    let mut child = cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().ok()?;
    // A saída é lida à parte: um comando que escreve muito não fica preso no cano cheio.
    let read = |mut pipe: Box<dyn Read + Send>| {
        std::thread::spawn(move || {
            let mut v = Vec::new();
            let _ = pipe.read_to_end(&mut v);
            v
        })
    };
    let out = read(Box::new(child.stdout.take()?));
    let err = read(Box::new(child.stderr.take()?));
    let deadline = Instant::now() + limit;
    let mut wait = Duration::from_millis(1);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Some(Output { status, stdout: out.join().unwrap_or_default(), stderr: err.join().unwrap_or_default() }),
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(wait);
                wait = (wait * 2).min(Duration::from_millis(10));
            }
            _ => {
                // Passou do limite (ou não deu para saber): encerra e não espera a leitura,
                // que pode estar presa num filho do comando.
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::output_within;
    use std::{
        process::Command,
        time::{Duration, Instant},
    };

    #[test]
    fn comando_que_termina() {
        let o = output_within(Command::new("sh").args(["-c", "echo oi; echo erro >&2; exit 3"]), Duration::from_secs(5)).unwrap();
        assert_eq!(o.stdout, b"oi\n");
        assert_eq!(o.stderr, b"erro\n");
        assert_eq!(o.status.code(), Some(3));
    }

    /// O caso do `bluetoothctl` sem o serviço: o comando nunca termina.
    #[test]
    fn comando_que_nao_termina() {
        let t = Instant::now();
        assert!(output_within(Command::new("sleep").arg("30"), Duration::from_millis(150)).is_none());
        assert!(t.elapsed() < Duration::from_secs(5), "{:?}", t.elapsed());
    }

    #[test]
    fn comando_que_nao_existe() {
        assert!(output_within(&mut Command::new("glove-comando-que-nao-existe"), Duration::from_secs(1)).is_none());
    }

    /// Mais saída do que cabe no cano (64 KiB): não trava esperando alguém ler.
    #[test]
    fn saida_grande() {
        let o = output_within(Command::new("sh").args(["-c", "head -c 300000 /dev/zero"]), Duration::from_secs(10)).unwrap();
        assert_eq!(o.stdout.len(), 300000);
    }
}
