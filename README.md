# Graphgen

Generate graph files by scraping `<a>` tags in url

```sh
graphgen https://en.wikipedia.org/wiki/Crocodile -d 2 -o crocodile -t 16
```

In dev:
```sh
cargo run --release -- https://en.wikipedia.org/wiki/Crocodile -d 3 -o crocodile_d3_undirected -k crocodile -t 16 --undirected
```