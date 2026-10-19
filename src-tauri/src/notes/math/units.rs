//! Purpose: Unit conversion for notes (`10 ft to m`, `72 f to c`): a table of units by kind and one parser for the line. Pure and offline; money is not here (rates need the network).
//! Contents: convert — parses `<number> <unit> to <unit>` and answers the value with the target's name; find — a unit by any of its spellings.

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Length,
    Mass,
    Volume,
    Area,
    Time,
    Temperature,
}

/// Names (the first is shown in answers), the kind and how many base units one is: metres, grams, litres, square metres, seconds. Temperature has no factor (it has an offset).
const UNITS: &[(&str, Kind, f64)] = &[
    ("mm|millimeter|millimeters", Kind::Length, 0.001),
    ("cm|centimeter|centimeters", Kind::Length, 0.01),
    ("m|meter|meters|metre|metres", Kind::Length, 1.0),
    ("km|kilometer|kilometers|kilometre|kilometres", Kind::Length, 1000.0),
    ("in|inch|inches", Kind::Length, 0.0254),
    ("ft|foot|feet", Kind::Length, 0.3048),
    ("yd|yard|yards", Kind::Length, 0.9144),
    ("mi|mile|miles", Kind::Length, 1609.344),
    ("nmi", Kind::Length, 1852.0),
    ("mg|milligram|milligrams", Kind::Mass, 0.001),
    ("g|gram|grams", Kind::Mass, 1.0),
    ("kg|kilogram|kilograms|kilo|kilos", Kind::Mass, 1000.0),
    ("oz|ounce|ounces", Kind::Mass, 28.349_523_125),
    ("lb|lbs|pound|pounds", Kind::Mass, 453.592_37),
    ("st|stone", Kind::Mass, 6_350.293_18),
    ("t|tonne|tonnes", Kind::Mass, 1_000_000.0),
    ("ml|milliliter|milliliters|millilitre|millilitres", Kind::Volume, 0.001),
    ("l|liter|liters|litre|litres", Kind::Volume, 1.0),
    ("tsp|teaspoon|teaspoons", Kind::Volume, 0.004_928_921_593_75),
    ("tbsp|tablespoon|tablespoons", Kind::Volume, 0.014_786_764_781_25),
    ("floz", Kind::Volume, 0.029_573_529_562_5),
    ("cup|cups", Kind::Volume, 0.236_588_236_5),
    ("pt|pint|pints", Kind::Volume, 0.473_176_473),
    ("qt|quart|quarts", Kind::Volume, 0.946_352_946),
    ("gal|gallon|gallons", Kind::Volume, 3.785_411_784),
    ("cm2|sqcm", Kind::Area, 0.0001),
    ("m2|sqm", Kind::Area, 1.0),
    ("km2|sqkm", Kind::Area, 1_000_000.0),
    ("in2|sqin", Kind::Area, 0.000_645_16),
    ("ft2|sqft", Kind::Area, 0.092_903_04),
    ("yd2|sqyd", Kind::Area, 0.836_127_36),
    ("ha|hectare|hectares", Kind::Area, 10_000.0),
    ("acre|acres", Kind::Area, 4_046.856_422_4),
    ("mi2|sqmi", Kind::Area, 2_589_988.110_336),
    ("ms|millisecond|milliseconds", Kind::Time, 0.001),
    ("s|sec|second|seconds", Kind::Time, 1.0),
    ("min|minute|minutes", Kind::Time, 60.0),
    ("h|hr|hour|hours", Kind::Time, 3600.0),
    ("d|day|days", Kind::Time, 86_400.0),
    ("wk|week|weeks", Kind::Time, 604_800.0),
    ("c|celsius", Kind::Temperature, 1.0),
    ("f|fahrenheit", Kind::Temperature, 1.0),
    ("k|kelvin", Kind::Temperature, 1.0),
];

/// The table row for a spelling (case does not matter, a degree sign is ignored).
fn find(name: &str) -> Option<&'static (&'static str, Kind, f64)> {
    let name = name.trim().trim_start_matches('°').to_lowercase();
    UNITS.iter().find(|(names, ..)| names.split('|').any(|n| n == name))
}

fn short(unit: &(&'static str, Kind, f64)) -> &'static str {
    unit.0.split('|').next().unwrap_or("")
}

fn temperature(value: f64, from: &str, to: &str) -> f64 {
    let kelvin = match from {
        "f" => (value - 32.0) * 5.0 / 9.0 + 273.15,
        "k" => value,
        _ => value + 273.15,
    };
    match to {
        "f" => (kelvin - 273.15) * 9.0 / 5.0 + 32.0,
        "k" => kelvin,
        _ => kelvin - 273.15,
    }
}

/// `<number> <unit> to|in|as <unit>` as the converted value and the target unit's short name; None when the line is anything else (or the units differ in kind).
pub fn convert(line: &str) -> Option<(f64, &'static str)> {
    let words: Vec<&str> = line.split_whitespace().collect();
    let at = words.iter().rposition(|w| matches!(w.to_lowercase().as_str(), "to" | "in" | "as"))?;
    let (left, right) = (&words[..at], &words[at + 1..]);
    let [to] = right else { return None };
    let (number, from) = match left {
        [n, unit] => (*n, *unit),
        [joined] => split_number(joined)?,
        _ => return None,
    };
    let (value, (from, to)) = (parse_number(number)?, (find(from)?, find(to)?));
    if from.1 != to.1 {
        return None;
    }
    let result = if from.1 == Kind::Temperature { temperature(value, short(from), short(to)) } else { value * from.2 / to.2 };
    result.is_finite().then(|| (result, short(to)))
}

/// `10ft` as ("10", "ft").
fn split_number(word: &str) -> Option<(&str, &str)> {
    let at = word.find(|c: char| c.is_alphabetic() || c == '°')?;
    (at > 0).then(|| word.split_at(at))
}

/// A number with optional thousands commas ("23,405" is 23405).
pub fn parse_number(text: &str) -> Option<f64> {
    text.replace(',', "").parse::<f64>().ok().filter(|v| v.is_finite())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conv(line: &str) -> Option<(String, &'static str)> {
        convert(line).map(|(v, u)| (format!("{v:.3}"), u))
    }

    #[test]
    fn converts_between_units_of_one_kind() {
        assert_eq!(conv("10 ft to m"), Some(("3.048".into(), "m")));
        assert_eq!(conv("10ft to m"), Some(("3.048".into(), "m")));
        assert_eq!(conv("5 km in mi"), Some(("3.107".into(), "mi")));
        assert_eq!(conv("1 hour as min"), Some(("60.000".into(), "min")));
        assert_eq!(conv("1,000 g to kg"), Some(("1.000".into(), "kg")));
    }

    #[test]
    fn temperature_has_an_offset() {
        assert_eq!(conv("212 f to c"), Some(("100.000".into(), "c")));
        assert_eq!(conv("0 °C to k"), Some(("273.150".into(), "k")));
    }

    #[test]
    fn refuses_what_it_cannot_convert() {
        assert_eq!(conv("10 kg to m"), None);
        assert_eq!(conv("10 usd to jpy"), None);
        assert_eq!(conv("ten ft to m"), None);
        assert_eq!(conv("10 ft to"), None);
    }
}
