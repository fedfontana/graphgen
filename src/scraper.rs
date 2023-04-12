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
    keywords: Option<Vec<String>>,
}

fn get_complete_url(url: &str) -> Option<String> {
    
    // All of the internal links start with a slash
    if !url.starts_with('/') {
        return Some(url.to_owned());
    }

    if url.contains(':') {
        return None;
    }

    if url.starts_with("/w") && !url.starts_with("/wiki") {
        return None;
    }

    if let Some((url, _tag)) = url.split_once("#") {
        return Some("https://en.wikipedia.org".to_owned() + url);
    }

    return Some("https://en.wikipedia.org".to_owned() + url);
}

impl<'a> WikipediaScraper<'a> {
    pub fn new(url: &'a str, depth: u64, keywords: Option<Vec<String>>) -> WikipediaScraper<'a> {
        if depth == 0 {
            panic!("Depth must be greater than 0");
        }

        WikipediaScraper {
            url,
            depth,
            links: HashSet::new(),
            pages: HashMap::new(),
            keywords,
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

        
        let Some(page_content)= self.get_page_content(start_url.as_ref())? else {
            eprintln!("Skipping {} with depth: {}", start_url.as_ref(), depth);
            return Ok(());
        };
        
        eprintln!("Scraping {} with depth: {}", start_url.as_ref(), depth);
    
        let anchor_list = get_anchor_list(&page_content)?;

        if anchor_list.is_empty() {
            return Ok(());
        }

        // If the page has already been visited, just add the links to the links set by recovering its id
        // else generate a new id and add it to the pages before proceeding to process the links
        let start_url_id = if let Some(start_url_id) = self.pages.get(start_url.as_ref()) {
            *start_url_id
        } else {
            let new_id = self.pages.len() as ID;
            self.pages.insert(start_url.as_ref().to_string(), new_id);
            new_id
        };

        for anchor in anchor_list {
            // If the link has already been visited, just add the current link to the links set
            if let Some(anchor_id) = self.pages.get(&anchor) {
                self.links.insert((start_url_id, *anchor_id));
            } else {
                // Else generate the anchor id and add it to the pages
                let anchor_id = self.pages.len() as ID;
                assert!(
                    self.pages.insert(anchor.clone(), anchor_id).is_none(),
                    "Should not be adding a page that already exists"
                );

                // Add the link
                assert!(
                    self.links.insert((start_url_id, anchor_id)),
                    "Should not be adding a link that already exists"
                );

                if anchor.starts_with("https://en.wikipedia.org/wiki/") {
                    // And then scrape that page recursively
                    self.scrape_with_depth(anchor, depth - 1)?;
                }
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

    fn get_page_content(&self, url: impl AsRef<str>) -> Result<Option<String>, Box<dyn Error>> {
        let mut resp = get(url.as_ref())?;
        let mut content = String::new();
        resp.read_to_string(&mut content)?;
    
        if let Some(keywords) = &self.keywords {
            let lower_content = content.to_lowercase();
            if keywords.iter().any(|keyword| lower_content.contains(keyword.to_lowercase().as_str())) {
                return Ok(Some(content));
            } else {
                return Ok(None);
            }
        }
        Ok(Some(content))
    }
}

fn get_anchor_list(page_content: &str) -> Result<Vec<String>, Box<dyn Error>> {
    let document = scraper::Html::parse_document(page_content);

    //TODO use errorkind and just log if we can't find the content (right now it is stopping the program)
    let content_selector = scraper::Selector::parse("#bodyContent")?;
    let content = document
        .select(&content_selector)
        .next();
    // .map_or_else(|| Err("No content found"), |content| Ok(content))?;
    if let Some(content) = content {        
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
    } else {
        eprintln!("No content found for a page");
        Ok(Vec::new())
    }
}