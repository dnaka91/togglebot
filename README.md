# 🤖 ToggleBot

[![Build Status][build-img]][build-url]
[![Twitch][twitch-img]][twitch-url]
[![Discord][discord-img]][discord-url]

[build-img]: https://img.shields.io/github/workflow/status/dnaka91/togglebot/CI/main?style=for-the-badge
[build-url]: https://github.com/dnaka91/togglebot/actions?query=workflow%3ACI
[twitch-img]: https://img.shields.io/badge/twitch-togglebit-9146ff?style=for-the-badge&logo=twitch&logoColor=white
[twitch-url]: https://twitch.tv/togglebit
[discord-img]: https://img.shields.io/badge/discord-togglebit-7289da?style=for-the-badge&logo=discord&logoColor=white
[discord-url]: https://twitch.tv/togglebit

This is the ToggleBot bot used on [togglebit](https://github.com/togglebyte)'s Discord server and
Twitch chat.

## Build

To build this project have `rust` and `cargo` available in the latest version and run `cargo build`.
Now you will find the binary at `target/debug/togglebot` which you can directly execute or use
`cargo run` for convenience.

### Docker

This bot is hosted on my private server in a Docker container that is build with the local
`Dockerfile`. The image is publicly available as `dnaka91/togglebot` and can be pulled with Docker.

To run the Docker image execute:

```sh
docker run -v $PWD/config.toml:/app/config.toml -v $PWD/temp:data dnaka91/togglebot
```

- `-v $PWD/config.toml:/app/config.toml` maps the local configuration at the proper place in the
  container for the bot to find it.
- `-v $PWD/temp:data` maps the data directory to a local folder which contains all state (like
  custom commands) for the bot.

## Configuration

The bot expect to find a config file named `config.toml` at the current working directory or at
`/app/config.toml` if the first one couldn't be found.

The following sections describe all configuration options of this bot.

### Discord

For Discord only a `token` is needed. This can be created by first adding a new application on TODO and then activating the bot feature. There should be a button in the bot area to get the token.

### Twitch

Twitch needs a `login` which is the user account and a `token` that can be generated at TODO. To
make a bot user a new normal user account needs to be created as Twitch doesn't have bot users as a
feature on its own.

### Example

Here is a short example of a full config file with sample values.

```toml
[discord]
token = "xxx"

[twitch]
login = "botname"
token = "xxx"
```

## License

This project is licensed under the [AGPL-3.0 License](LICENSE) (or
<https://www.gnu.org/licenses/agpl-3.0.html>).
