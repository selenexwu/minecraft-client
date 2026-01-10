use anyhow::Result;
use minecraft_client::connection::Connection;

fn main() -> Result<()> {
    let host = "localhost";
    // let host = "play.budpe.com";
    let port = 25565;
    let mut conn = Connection::connect(host.to_string(), port)?;

    // let status = conn.get_status()?;
    // println!("{}", status);

    conn.login()?;
    conn.configure()?;
    conn.play()?;

    Ok(())
}
