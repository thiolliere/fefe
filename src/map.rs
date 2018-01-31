use rand::distributions::{IndependentSample, Weighted, WeightedChoice};
use std::path::PathBuf;
use std::fs::File;
use std::io::Read;
use lyon::svg::parser::FromSpan;
use lyon::svg::parser::svg::{Tokenizer, Token};
use lyon::svg::parser::{ElementId, AttributeId};
use lyon::svg::parser::svg::Name::Svg;
use lyon::svg::parser::svg::ElementEnd::Close;
use lyon::svg::parser::svg::ElementEnd::Empty;
use lyon::svg::path::default::Path;
use entity::{EntityPosition, EntitySettings};

pub fn load_map(path: PathBuf, world: &mut ::specs::World) -> Result<(), ::failure::Error> {
    let mut settings_path = path.clone();
    settings_path.set_extension("ron");
    let settings_file = File::open(&settings_path)
        .map_err(|e| format_err!("\"{}\": {}", settings_path.to_string_lossy(), e))?;
    let mut settings: MapSettings = ::ron::de::from_reader(settings_file)
        .map_err(|e| format_err!("\"{}\": {}", settings_path.to_string_lossy(), e))?;

    let mut svg_path = path.clone();
    svg_path.set_extension("svg");
    let mut svg_file = File::open(&svg_path)
        .map_err(|e| format_err!("\"{}\": {}", svg_path.to_string_lossy(), e))?;
    let mut svg_string = String::new();
    svg_file.read_to_string(&mut svg_string)
        .map_err(|e| format_err!("\"{}\": {}", svg_path.to_string_lossy(), e))?;

    let mut rules_entities = settings.rules.iter().map(|_| vec![]).collect::<Vec<_>>();

    let mut tokenizer = Tokenizer::from_str(&svg_string);

    let mut in_marker = false;
    let mut style = None;
    let mut d = None;
    let mut in_path_attribute = false;
    while let Some(token) = tokenizer.next() {
        let token = token
            .map_err(|e| format_err!("\"{}\": {}", svg_path.to_string_lossy(), e))?;

        // Ignore markers
        if in_marker {
            match token {
                Token::ElementEnd(Close(Svg(ElementId::Marker))) => {
                    in_marker = false;
                }
                _ => (),
            }
        // Process path attribute
        } else if in_path_attribute {
            match token {
                Token::Attribute(Svg(AttributeId::Style), value) => {
                    style = Some(value);
                }
                Token::Attribute(Svg(AttributeId::D), value) => {
                    d = Some(value);
                }
                Token::ElementEnd(Empty) => {
                    if let (Some(style), Some(d)) = (style, d) {
                        for (rule, ref mut rule_entities) in settings.rules.iter().zip(rules_entities.iter_mut()) {
                            if style.to_str().contains(&rule.trigger) {
                                let path = load_entity_position(d.to_str())
                                    .map_err(|e| format_err!("\"{}\": {}", svg_path.to_string_lossy(), e))?;
                                rule_entities.push(path);
                            }
                        }
                    }
                    in_path_attribute = false;
                }
                _ => (),
            }
        } else {
            match token {
                Token::ElementStart(Svg(ElementId::Marker)) => {
                    in_marker = true;
                }
                Token::ElementStart(Svg(ElementId::Path)) => {
                    in_path_attribute = true;
                    style = None;
                    d = None;
                }
                _ => (),
            }
        }
    }

    // Insert entities to world
    for (rule, rule_entities) in settings.rules.drain(..).zip(rules_entities.drain(..)) {
        rule.inserter.insert(rule_entities, world);
    }
    Ok(())
}

fn load_entity_position(commands: &str) -> Result<EntityPosition, ::failure::Error> {
    let svg_builder = Path::builder().with_svg();
    ::lyon::svg::path_utils::build_path(svg_builder, commands)
        .map_err(|e| format_err!("invalid path \"{}\": {:?}", commands, e))
}

#[derive(Serialize, Deserialize)]
struct MapSettings {
    rules: Vec<Rule>,
}

#[derive(Serialize, Deserialize)]
struct Rule {
    trigger: String,
    inserter: Inserter,
}

#[derive(Serialize, Deserialize)]
/// entities are randomized before being processed
enum Inserter {
    InsertEntity(EntitySettings),
    TakeNInsertion(usize, Box<Inserter>),
    RandomInsertionDispatch(Vec<(u32, Box<Inserter>)>),
    OrdonateInsertionDispatch(Vec<Box<Inserter>>),
}

impl Inserter {
    fn insert(self, mut entities: Vec<EntityPosition>, world: &mut ::specs::World) {
        use self::Inserter::*;
        match self {
            InsertEntity(entity_settings) => {
                for entity in entities {
                    entity_settings.insert(entity, world);
                }
            },
            TakeNInsertion(n, inserter) => {
                entities.truncate(n);
                inserter.insert(entities, world);
            },
            RandomInsertionDispatch(weighted_inserters) => {
                let mut rng = ::rand::thread_rng();
                let mut inserters_entities = weighted_inserters.iter().map(|_| vec![]).collect::<Vec<_>>();
                let mut items = weighted_inserters.iter()
                    .enumerate()
                    .map(|(item, &(weight, _))| Weighted { weight, item })
                    .collect::<Vec<_>>();
                let choices = WeightedChoice::new(&mut items);

                for entity in entities {
                    let i = choices.ind_sample(&mut rng);
                    inserters_entities[i].push(entity);
                }
                for (_, inserter) in weighted_inserters {
                    inserter.insert(inserters_entities.remove(0), world)
                }
            },
            OrdonateInsertionDispatch(inserters) => {
                let mut inserters_entities = inserters.iter().map(|_| vec![]).collect::<Vec<_>>();
                let mut i = 0;
                for entity in entities {
                    inserters_entities[i].push(entity);
                    i += 1;
                    i %= inserters_entities.len();
                }
                for inserter in inserters {
                    inserter.insert(inserters_entities.remove(0), world)
                }
            },
        }
    }
}
