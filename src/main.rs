extern crate pest;
#[macro_use] extern crate pest_derive;

mod parser {
    use {Entity, Attribute, Relationship, Cardinality};
    use pest;
    use pest::Parser;

    #[cfg(debug_assertions)]
    const _GRAMMAR : &'static str = include_str!("grammar.pest");

    #[derive(Parser)]
    #[grammar = "grammar.pest"]
    struct EntityParser;

    fn parse_cardinality(pair : pest::iterators::Pair<Rule, pest::inputs::StrInput>) -> Cardinality {
        use Cardinality::*;
        assert_eq!(Rule::card, pair.as_rule());
        let card = pair.into_inner().next().unwrap();
        match card.as_rule() {
            Rule::card_any => AtLeast(0),
            Rule::card_exact => Exactly(card.as_str().parse().unwrap()),
            Rule::card_at_least => AtLeast(card.into_inner().next().unwrap().as_str().parse().unwrap()),
            Rule::card_range => {
                let mut iter = card.into_inner();
                let n = iter.next().unwrap().as_str().parse().unwrap();
                let m = iter.next().unwrap().as_str().parse().unwrap();
                Range(n, m)
            },
            _ => unreachable!("{:?}", card.as_rule()),
        }
    }

    fn parse_relationship(pair : pest::iterators::Pair<Rule, pest::inputs::StrInput>) -> Relationship {
        assert_eq!(Rule::relationship, pair.as_rule());
        let rel = pair.into_inner().next().unwrap();
        match rel.as_rule() {
            Rule::binary_rel => {
                let mut iter = rel.into_inner();
                let e1 = iter.next().unwrap().as_str().to_owned();
                let s1 = iter.next().unwrap().as_str().to_owned();
                let c1 = parse_cardinality(iter.next().unwrap());
                let c2 = parse_cardinality(iter.next().unwrap());
                let e2 = iter.next().unwrap().as_str().to_owned();
                let s2 = iter.next().unwrap().as_str().to_owned();
                Relationship::Binary(e1, s1, c1, e2, s2, c2)
            },
            Rule::subtype_open => {
                let mut iter = rel.into_inner();
                let name = iter.next().unwrap().as_str().to_owned();
                let subs = iter.map(|p| p.as_str().to_owned()).collect();
                Relationship::SubTypeOpen(name, subs)
            },
            Rule::subtype_closed => {
                let mut iter = rel.into_inner();
                let name = iter.next().unwrap().as_str().to_owned();
                let disc = iter.next().unwrap().as_str().to_owned();
                let subs = iter.map(|p| p.as_str().to_owned()).collect();
                Relationship::SubTypeClosed(name, disc, subs)
            },
            _ => unreachable!("{:?}", rel.as_rule()),
        }
    }

    fn parse_entity(pair : pest::iterators::Pair<Rule, pest::inputs::StrInput>) -> Entity {
        assert_eq!(Rule::entity, pair.as_rule());
        let mut iter = pair.into_inner().peekable();
        assert_eq!(Rule::name, iter.peek().unwrap().as_rule());
        let name = iter.next().unwrap().as_str().to_owned();
        let mut attributes = Vec::new();
        let mut independent = true;
        for p in iter {
            assert_eq!(Rule::attribute, p.as_rule());
            let mut attr = p.into_inner().peekable();
            assert_eq!(Rule::name, attr.peek().unwrap().as_rule());
            let mut name = attr.next().unwrap().as_str().to_owned();
            if attr.peek().map(|p| p.as_rule()) == Some(Rule:: name) {
                name += " / ";
                name += attr.next().unwrap().as_str();
            }
            let mut a = Attribute::new(&name);
            while let Some(m) = attr.next() {
                match m.as_rule() {
                    Rule::mod_fk => { a = a.fk() },
                    Rule::mod_null => { a = a.null() },
                    Rule::mod_pk => { a = a.pk() },
                    Rule::mod_ak => {
                        let ak = m.into_inner().next().unwrap().as_str().parse().unwrap();
                        a = a.ak(ak)
                    },
                    _ => unreachable!("{:?}", m.as_rule()),
                }
            }
            if a.in_fk && a.in_pk {
                independent = false;
            }
            attributes.push(a);
        }
        Entity { name, attributes, independent }
    }

    pub fn parse_model(input : &str) -> (Vec<Entity>, Vec<Relationship>) {
        let mut entities = Vec::new();
        let mut relationships = Vec::new();
        let result = EntityParser::parse_str(Rule::model, input).unwrap();
        for pair in result {
            if pair.as_rule() == Rule::entity {
                entities.push(parse_entity(pair));
            } else if pair.as_rule() == Rule::relationship {
                relationships.push(parse_relationship(pair));
            }
        }
        (entities, relationships)
    }

    #[test]
    fn test_entity_parser() {
        use write_graph;
        let input = r#"
[species]
species_id :pk
scientific_name :ak1
diet
can_move

[stationary_species]
species_id :pk :fk

[moving_species]
species_id :pk :fk
limbs

species =(can_move) stationary_species + moving_species

[crawling_species]
species_id :pk :fk
ground_speed
can_climb
can_dig

[flying_species]
species_id :pk :fk
flying_speed
max_altitude

[swimming_species]
species_id :pk :fk
swim_speed
max_depth

moving_species >: crawling_species + flying_species + swimming_species

[biome]
biome_id :pk
temperature
biome_type

[surface_biome]
biome_id :pk :fk
altitude
humidity

[ocean_biome]
biome_id :pk :fk
depth
salinity

[underground_biome]
biome_id :pk :fk

biome =(biome_type) surface_biome + ocean_biome + underground_biome

species "lives in" 1+:1+ biome "supports"

[predation]
predator/species_id :pk :fk
prey/species_id :pk :fk
biome_id :pk :fk

species "is predator" 1:* predation "has predator"
species "is prey" 1:* predation "has prey"
biome "facilitates" 1:* predation "occurs in"
"#;
        let (entities, relationships) = parse_model(input);
        write_graph(&entities, &relationships);
    }
}

type KeyFlag = u32;
type Name = String;

#[derive(Clone)]
struct Attribute {
    name : Name,
    in_pk : bool,
    in_fk : bool,
    nullable : bool,
    aks : KeyFlag,
}
impl Attribute {
    fn new(name : &str) -> Attribute {
        Attribute {
            name: name.to_owned(),
            in_pk: false,
            in_fk: false,
            nullable: false,
            aks: 0
        }
    }
    fn fk(mut self) -> Self { self.in_fk = true; self }
    fn pk(mut self) -> Self { self.in_pk = true; self }
    fn null(mut self) -> Self { self.nullable = true; self }
    fn ak(mut self, id : u8) -> Self { self.aks ^= 1 << (id - 1); self }
}

pub struct Entity {
    name : Name,
    attributes : Vec<Attribute>,
    independent : bool,
}
impl Entity {
    fn new(name : &str) -> Entity {
        Entity {
            name: name.to_owned(),
            attributes: Vec::new(),
            independent: true,
        }
    }
    fn att(mut self, a : Attribute) -> Self {
        self.attributes.push(a);
        self
    }
}

#[derive(Clone, Copy)]
pub enum Cardinality {
    Exactly(usize),
    Range(usize, usize),
    AtLeast(usize),
}
use Cardinality::*;

pub enum Relationship {
    Binary(Name, String, Cardinality, Name, String, Cardinality),
    SubTypeOpen(Name, Vec<Name>),
    SubTypeClosed(Name, Name, Vec<Name>),
}
use Relationship::*;

fn write_graph(entities : &[Entity], relationships : &[Relationship]) {
    println!("digraph {{");
    println!("node [shape=plain]");
    let print_attributes = |atts : &[Attribute]| {
        for a in atts.iter() {
            print!("<tr><td align=\"left\">{}</td><td align=\"left\">", a.name);
            let mut modifiers = Vec::new();
            if a.in_fk {
                modifiers.push("FK".to_owned());
            }
            {
                let aks = a.aks;
                for i in 0..31 {
                    if aks & (1 << i) != 0 {
                        modifiers.push(format!("AK{}", i + 1));
                    }
                }
            }
            if a.nullable {
                modifiers.push("NULL".to_owned());
            }
            if ! modifiers.is_empty() {
                print!("<font point-size=\"8\">");
                for m in &modifiers {
                    print!("{} ", m);
                }
                print!("</font>");
            }
            println!("</td></tr>");
        }
    };
    for e in entities {
        println!(r#"{name} [label=<<table cellspacing="0" border="0"><tr><td align="left">{name}</td></tr><tr><td><table cellborder="0" {style}>"#,
                 name = e.name,
                 style = if e.independent { "" } else {"style=\"rounded\""}
                 );
        let (pk, nk) : (Vec<Attribute>, Vec<Attribute>) = e.attributes.clone().into_iter().partition(|a| a.in_pk);
        print_attributes(&pk);
        if ! nk.is_empty() {
            println!("<hr />");
        }
        print_attributes(&nk);
        println!("</table></td></tr></table>\n>]");
    }
    println!("splines=\"ortho\"");
    println!("edge [arrowhead=\"none\" color=\"gray\"]");
    let mut subid = 1;
    for r in relationships {
        match *r {
            Binary(ref e1, ref s1, c1, ref e2, ref s2, c2) => {
                let print_card = |c : Cardinality| -> String { match c {
                    Exactly(1) | Range(1, 1) => String::from(""),
                    Exactly(n) => format!("{}", n),
                    AtLeast(0) => String::from("∗"),
                    AtLeast(n) => format!("{}+", n),
                    Range(n, m) => format!("{}..{}", n, m),
                }};
                println!(r#"{} -> {} [taillabel="{}"] [headlabel="{}"] [label=<<font point-size="10">{} / {}</font>>]"#, e1, e2, print_card(c1), print_card(c2), s1, s2);
            },
            SubTypeClosed(ref sup, ref disc, ref subs) => {
                println!(r#"subtype{} [shape="doublecircle" label=<<font point-size="10">{}</font>> width=0.2 margin=0]"#, subid, disc);
                println!("{} -> subtype{}", sup, subid);
                for sub in subs.iter() {
                    println!("subtype{} -> {}", subid, sub);
                }
                subid += 1;
            },
            SubTypeOpen(ref sup, ref subs) => {
                println!(r#"subtype{} [shape="circle" label="" width=0.2]"#, subid);
                println!("{} -> subtype{}", sup, subid);
                for sub in subs.iter() {
                    println!("subtype{} -> {}", subid, sub);
                }
                subid += 1;
            },
        }
    }
    println!("}}");
}

fn main() {
    use std::io::Read;
    let mut input = String::new();
    if std::io::stdin().read_to_string(&mut input).is_ok() {
        let (entities, relationships) = parser::parse_model(&input);
        write_graph(&entities, &relationships);
    }
}
