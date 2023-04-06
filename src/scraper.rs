use reqwest::blocking::get;
use std::{
    collections::{HashMap, HashSet},
    error::Error,
    io::{Read, Write},
};

type ID = u64;

pub struct WikipediaScraper<'a> {
    url: &'a str,
    depth: u64,
    links: HashSet<(ID, ID)>,
    pages: HashMap<String, ID>,
}

//TODO filter out images/catergories/other links that don't lead to a page

//TODO some links redirect to the same page, so we need to check for that before adding them to the set
//TODO maybe build a map of links that redirect to the same page and check against that instead
//TODO of making requests each time
fn get_complete_url(url: &str) -> Option<String> {
    // All of the internal links start with a slash
    if url.starts_with('/') {
        return Some("https://en.wikipedia.org".to_owned() + url);
    }
    None
}

impl<'a> WikipediaScraper<'a> {
    pub fn new(url: &'a str, depth: u64) -> WikipediaScraper<'a> {
        if depth == 0 {
            panic!("Depth must be greater than 0");
        }

        WikipediaScraper {
            url,
            depth,
            links: HashSet::new(),
            pages: HashMap::new(),
        }
    }

    pub fn scrape(&mut self) -> Result<(), Box<dyn Error>> {
        self.scrape_with_depth(self.url, self.depth)
    }

    fn scrape_with_depth(
        &mut self,
        start_url: impl AsRef<str>,
        depth: u64,
    ) -> Result<(), Box<dyn Error>> {
        if depth == 0 {
            return Ok(());
        }

        let page_content = get_page_content(self.url)?;
        let anchor_list = get_anchor_list(&page_content)?;

        // TODO: filter anchor list based on some heuristic (that should be encapsulated in a function)
        for anchor in anchor_list {
            let start_url_id = self.pages.len() as ID;
            self.pages
                .insert(start_url.as_ref().to_string(), start_url_id);

            // If the link has already been visited, just add the current link to the links set
            if let Some(anchor_id) = self.pages.get(&anchor) {
                self.links.insert((start_url_id, *anchor_id));
            } else {
                // Else generate the anchor id and add it to the pages
                let anchor_id = self.pages.len() as ID;
                self.pages.insert(anchor.clone(), anchor_id);

                // Add the link
                self.links.insert((start_url_id, anchor_id));

                // And then scrape that page recursively
                self.scrape_with_depth(anchor, depth - 1)?;
            }
        }

        Ok(())
    }

    pub fn save_to_file(&self, output_file: impl AsRef<str>) -> Result<(), Box<dyn Error>> {
        let edges_file_path = format!("{}_edges.csv", output_file.as_ref());
        let nodes_file_path = format!("{}_nodes.csv", output_file.as_ref());

        let mut edges_file = std::fs::File::create(edges_file_path)?;
        let mut nodes_file = std::fs::File::create(nodes_file_path)?;

        edges_file.write_all("source,target\n".as_bytes())?;
        nodes_file.write_all("node_id,url\n".as_bytes())?;

        for page in self.pages() {
            nodes_file.write_all(format!("{},\"{}\"\n", page.1, page.0).as_bytes())?;
        }

        for link in self.links() {
            edges_file.write_all(format!("{},{}\n", link.0, link.1).as_bytes())?;
        }

        Ok(())
    }

    pub fn links(&self) -> impl Iterator<Item = &(ID, ID)> {
        self.links.iter()
    }

    pub fn pages(&self) -> impl Iterator<Item = (&String, &ID)> {
        self.pages.iter()
    }
}

fn get_page_content(url: impl AsRef<str>) -> Result<String, Box<dyn Error>> {
    let mut resp = get(url.as_ref())?;
    let mut content = String::new();
    resp.read_to_string(&mut content)?;
    Ok(content)
}

fn get_anchor_list(page_content: &str) -> Result<Vec<String>, Box<dyn Error>> {
    let document = scraper::Html::parse_document(page_content);

    //TODO use errorkind and just log if we can't find the content (right now it is stopping the program)
    let content_selector = scraper::Selector::parse("#bodyContent")?;
    let content = document
        .select(&content_selector)
        .next()
        .map_or_else(|| Err("No content found"), |content| Ok(content))?;

    let anchor_selector = scraper::Selector::parse("a")?;

    let anchors = content.select(&anchor_selector);
    let mut anchor_list = Vec::new();
    for anchor in anchors {
        if let Some(href) = anchor.value().attr("href") {
            if let Some(url) = get_complete_url(href) {
                anchor_list.push(url);
            }
        }
    }

    Ok(anchor_list)
}
