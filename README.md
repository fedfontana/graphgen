# Graphgen

Generate graph files by scraping `<a>` tags in url

```sh
graphgen https://en.wikipedia.org/wiki/Crocodile -d 20 -o crocodile -t 8
```

In dev:
```sh
cargo run -- https://en.wikipedia.org/wiki/Crocodile -d 3 -o crocodile -k crocodile -t 8
```

Run cargo-flamegraph:
```sh
time CARGO_PROFILE_RELEASE_DEBUG=true cargo flamegraph --root -- https://en.wikipedia.org/wiki/Crocodile -d 2 -k crocodile -o prova_flamegraph -t 16
```