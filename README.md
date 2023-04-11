# Graphgen

Generate graph files by scraping `<a>` tags in url

```sh
graphgen https://en.wikipedia.org/wiki/Crocodile -d 20 -o crocodile
```

In dev:
```sh
cargo run -- https://en.wikipedia.org/wiki/Crocodile -d 3 -o crocodile -k crocodile
```