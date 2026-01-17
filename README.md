[![](https://img.shields.io/badge/bitsy_1.0.0-passing-green)](https://github.com/gongahkia/bitsy/releases/tag/1.0.0)

# `Bitsy`

[Vim](https://www.vim.org/)-compatible text editor written in Rust.

<div align="center">
    <img src="./asset/reference/1.png" width="50%">"
</div>

## Stack

* *Script*: [Rust](https://www.rust-lang.org/)
* *CLI rendering*: [crossterm](https://github.com/crossterm-rs/crossterm), [ropey](https://github.com/cessen/ropey), [notify](https://github.com/notify-rs/notify)
* *Errors*: [anyhow](https://github.com/dtolnay/anyhow), [log](https://github.com/rust-lang/log), [env_logger](https://github.com/env-logger-rs/env_logger)
* *Serialization and Deserialization*: [serde](https://github.com/serde-rs/serde), [toml](https://github.com/toml-rs/toml)
* *Encoding*: [encoding_rs](https://github.com/hsivonen/encoding_rs), [chardetng](https://github.com/hsivonen/chardetng)
* *Search*: [regex](https://github.com/rust-lang/regex), [walkdir](https://github.com/BurntSushi/walkdir)
* *Clipboard*: [arboard](https://github.com/1Password/arboard)

## Video

As per custom, here is a video of `Bitsy` editing its own [source code](./src/).

https://github.com/user-attachments/assets/a1a374d2-8965-4a50-bc84-6001d7682468

## Other screenshots

<div align="center">
    <img src="./asset/reference/2.png" width="45%">"
    <img src="./asset/reference/3.png" width="45%">"
</div>

## Usage

The below instructions are for locally building `Bitsy`.

1. First run the below commands.

```console
$ git clone https://github.com/gongahkia/bitsy && cd bitsy
$ cargo build --release # build bitsy for production
$ cargo install --path # install bitsy locally
```

2. Then get started with `Bisty` with the following

```console
$ bitsy # open landing page
$ bitsy myfile.txt # edit an existing file
```

3. `Bitsy` additionally provides the below commands.

* `:help` in Command Mode: *Pulls up a user manual in the current buffer*
* `Ctrl + p`: *Fuzzy Finder that searches within the current directory*

## Architecture

```mermaid
graph TD
    subgraph "main.rs"
        A[main] --> B{Editor::new};
    end

    subgraph "editor.rs"
        B --> C[Editor Struct];
        C --> D{run};
        D --> E{handle_event};
        E --> F{handle_key};
        F --> G{execute_action};
        F --> H{handle_command_mode_key};
        H --> I{execute_command};
    end

    subgraph "Core Components"
        C -- contains --> J[Terminal];
        C -- contains --> K[Vec<Buffer>];
        C -- contains --> L[Mode];
        C -- contains --> M[StatusLine];
        C -- contains --> N[CommandBar];
        C -- contains --> O[RegisterManager];
        C -- contains --> P[Vec<Window>];
        C -- contains --> Q[FuzzyFinder];
    end

    subgraph "Data Structures"
        K -- uses --> R[ropey::Rope];
        O -- contains --> S[HashMap<char, RegisterContent>];
    end

    subgraph "Dependencies"
        J -- uses --> T[crossterm];
        I -- uses --> U[pulldown-cmark];
        I -- uses --> V[tiny_http];
        I -- uses --> W[webbrowser];
        Q -- uses --> X[walkdir];
    end
```

## Other notes

`Bitsy` is also the spiritual successor of [`Shed`](https://github.com/gongahkia/shed), a *much-worse* text editor I wrote at the beginning of my programming journey.
