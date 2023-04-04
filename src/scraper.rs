use reqwest::blocking::get;
use std::{collections::HashSet, error::Error, io::{Read, Write}};

pub struct WikipediaScraper<'a> {
    url: &'a str,
    depth: u64,
    links: HashSet<(String, String)>,
    pages: HashSet<String>,
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
            pages: HashSet::new(),
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

        for anchor in anchor_list {
            self.links
                .insert((start_url.as_ref().to_string().clone(), anchor.clone()));
            // if we've already visited this node, skip it
            if !self.pages.contains(&*anchor) {
                self.pages.insert(anchor.clone());
                self.scrape_with_depth(anchor, depth - 1)?;
            }
        }
        Ok(())
    }

    pub fn save_to_file(&self, output_file: impl AsRef<str>) -> Result<(), Box<dyn Error>> {
        let mut file = std::fs::File::create(output_file.as_ref())?;
        file.write("source,target\n".as_bytes())?;
        for link in self.links() {
            file.write_all(format!("{},{}\n", link.0, link.1).as_bytes())?;
        }
        Ok(())
    }

    pub fn links(&self) -> impl Iterator<Item = &(String, String)> {
        self.links.iter()
    }

    pub fn pages(&self) -> impl Iterator<Item = &String> {
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
