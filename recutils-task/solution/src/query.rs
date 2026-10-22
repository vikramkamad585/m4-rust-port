// rec_db_query: selection, sorting, grouping, joins, field expressions
// and aggregate functions.

use crate::fex::FexElem;
use crate::rec::{Db, Elem, Field, RElem, Record, Rset};
use crate::sex::Sex;
use crate::types::{atod, values_cmp, Props};
use std::cmp::Ordering;

pub struct Query<'a> {
    pub rtype: Option<String>,
    pub join: Option<String>,
    pub index: Option<Vec<(usize, Option<usize>)>>,
    pub sex: Option<&'a Sex>,
    pub quick: Option<String>,
    pub fex: Option<Vec<FexElem>>,
    pub group_by: Option<Vec<String>>,
    pub sort_by: Option<Vec<String>>,
    pub icase: bool,
    pub descriptor: bool,
    pub uniq: bool,
}

/// rec_rset_compare_typed_records.
fn compare_typed(props: &Props, r1: &Record, r2: &Record, fields: &[String]) -> i32 {
    let mut result = 0;
    for name in fields {
        let f1 = r1.get(name, 0);
        let f2 = r2.get(name, 0);
        match (f1, f2) {
            (Some(_), None) => return 1,
            (None, Some(_)) => return -1,
            (None, None) => return -1,
            (Some(a), Some(b)) => {
                let k = props.field_type(name);
                result = match values_cmp(k.as_ref(), &a.value, &b.value) {
                    Ordering::Less => -1,
                    Ordering::Equal => 0,
                    Ordering::Greater => 1,
                };
                if result != 0 {
                    break;
                }
            }
        }
    }
    result
}

/// rec_rset_sort: re-insert every element with gnulib's sorted-list
/// binary insertion, using the record comparator (equal compares as less).
pub fn sort_rset(rset: &mut Rset, props: &Props, order: &[String]) {
    let old = std::mem::take(&mut rset.elems);
    let mut list: Vec<RElem> = Vec::with_capacity(old.len());
    for elt in old {
        let mut low = 0usize;
        let mut high = list.len();
        while low < high {
            let mid = low + (high - low) / 2;
            let cmp = match (&list[mid], &elt) {
                (RElem::Comment(_), _) => -1,
                (RElem::Record(_), RElem::Comment(_)) => 1,
                (RElem::Record(a), RElem::Record(b)) => {
                    let c = compare_typed(props, a, b, order);
                    if c == 0 {
                        -1
                    } else {
                        c
                    }
                }
            };
            if cmp < 0 {
                low = mid + 1;
            } else if cmp > 0 {
                high = mid;
            } else {
                low = mid;
                break;
            }
        }
        list.insert(low, elt);
    }
    rset.elems = list;
}

fn group_rset(rset: &mut Rset, props: &Props, group_by: &[String]) {
    let n = rset.num_records();
    let mut deletion = vec![false; n];
    let rec_idx: Vec<usize> = rset
        .elems
        .iter()
        .enumerate()
        .filter(|(_, e)| matches!(e, RElem::Record(_)))
        .map(|(i, _)| i)
        .collect();
    for a in 0..n {
        if deletion[a] {
            continue;
        }
        let mut b = a + 1;
        while b < n {
            let (ra, rb) = {
                let ea = &rset.elems[rec_idx[a]];
                let eb = &rset.elems[rec_idx[b]];
                match (ea, eb) {
                    (RElem::Record(x), RElem::Record(y)) => (x.clone(), y.clone()),
                    _ => unreachable!(),
                }
            };
            if compare_typed(props, &ra, &rb, group_by) != 0 {
                break;
            }
            // merge rb into ra (skipping group-by fields)
            if let RElem::Record(target) = &mut rset.elems[rec_idx[a]] {
                for e in &rb.elems {
                    match e {
                        Elem::Field(f) => {
                            if group_by.contains(&f.name) {
                                continue;
                            }
                            target.elems.push(Elem::Field(f.clone()));
                        }
                        Elem::Comment(c) => target.elems.push(Elem::Comment(c.clone())),
                    }
                }
            }
            deletion[b] = true;
            b += 1;
        }
    }
    let mut k = 0;
    let mut kept = Vec::new();
    for e in std::mem::take(&mut rset.elems) {
        match e {
            RElem::Record(r) => {
                if !deletion[k] {
                    kept.push(RElem::Record(r));
                }
                k += 1;
            }
            c => kept.push(c),
        }
    }
    rset.elems = kept;
}

fn join(db: &Db, t1: &str, field: &str, t2: &str) -> Option<Rset> {
    let r1 = &db.rsets[db.get_by_type(Some(t1))?];
    let r2 = &db.rsets[db.get_by_type(Some(t2))?];
    let key = Props::from_rset(r2).key?;
    let mut out = Rset::default();
    for rec1 in r1.records() {
        let nfk = rec1.count(field);
        for k in 0..nfk {
            for rec2 in r2.records() {
                let key_field = match rec2.get(&key, k) {
                    Some(f) => f,
                    None => break,
                };
                let found = (0..rec1.count(field)).any(|i| rec1.get(field, i).unwrap().value == key_field.value);
                if !found {
                    continue;
                }
                let mut merged = rec1.clone();
                for f in rec2.fields() {
                    let mut nf = f.clone();
                    nf.name = format!("{}_{}", field, f.name);
                    merged.push_field(nf);
                }
                merged.remove_all(field);
                out.elems.push(RElem::Record(merged));
            }
        }
    }
    let mut d = Record::default();
    d.push_field(Field::new("%rec", &format!("{}_{}", t1, field)));
    out.descriptor = Some(d);
    Some(out)
}

pub fn fmt_g(v: f64) -> String {
    crate::cfmt::format_g(v, 6)
}

fn agg_record(func: &str, r: &Record, field: &str) -> f64 {
    let vals: Vec<f64> = r.fields().filter(|f| f.name == field).filter_map(|f| atod(&f.value)).collect();
    match func {
        "avg" => {
            if vals.is_empty() {
                0.0
            } else {
                vals.iter().sum::<f64>() / vals.len() as f64
            }
        }
        "sum" => vals.iter().fold(0.0, |a, b| a + b),
        "min" => vals.iter().fold(f64::MAX, |a, &b| if a < b { a } else { b }),
        "max" => vals.iter().fold(f64::MIN_POSITIVE, |a, &b| if a > b { a } else { b }),
        _ => 0.0,
    }
}

fn aggregate(func: &str, rset: &Rset, record: Option<&Record>, field: &str) -> String {
    let f = func.to_ascii_lowercase();
    if f == "count" {
        let n = match record {
            Some(r) => r.count(field),
            None => rset.records().map(|r| r.count(field)).sum(),
        };
        return n.to_string();
    }
    let v = match record {
        Some(r) => agg_record(&f, r, field),
        None => match f.as_str() {
            "avg" => {
                let mut avg = 0.0;
                let mut n = 0usize;
                for r in rset.records() {
                    avg += agg_record("avg", r, field);
                    n += 1;
                }
                if n != 0 {
                    avg /= n as f64;
                }
                avg
            }
            "sum" => rset.records().fold(0.0, |a, r| a + agg_record("sum", r, field)),
            "min" => rset.records().fold(f64::MAX, |a, r| {
                let b = agg_record("min", r, field);
                if a < b {
                    a
                } else {
                    b
                }
            }),
            _ => rset.records().fold(f64::MIN_POSITIVE, |a, r| {
                let b = agg_record("max", r, field);
                if a > b {
                    a
                } else {
                    b
                }
            }),
        },
    };
    fmt_g(v)
}

pub fn known_aggregate(name: &str) -> bool {
    matches!(name.to_ascii_lowercase().as_str(), "count" | "avg" | "sum" | "min" | "max")
}

fn process_fex(rset: &Rset, record: Option<&Record>, fex: &Option<Vec<FexElem>>) -> Record {
    let fex = match fex {
        None => return record.cloned().unwrap_or_default(),
        Some(f) => f,
    };
    let mut res = Record::default();
    for e in fex {
        if let Some(func) = &e.function {
            if known_aggregate(func) {
                let v = aggregate(func, rset, record, &e.field);
                let name = match &e.alias {
                    Some(a) => a.clone(),
                    None => format!("{}_{}", func, e.field),
                };
                res.push_field(Field::new(&name, &v));
            }
        } else {
            let r = match record {
                Some(r) => r,
                None => continue,
            };
            let (min, max): (i64, i64) = if e.min == -1 && e.max == -1 {
                (0, r.count(&e.field) as i64)
            } else if e.max == -1 {
                (e.min, e.min + 1)
            } else {
                (e.min, e.max + 1)
            };
            let mut j = min;
            while j < max {
                if j >= 0 {
                    if let Some(f) = r.get(&e.field, j as usize) {
                        let mut nf = f.clone();
                        if let Some(a) = &e.alias {
                            nf.name = a.clone();
                        }
                        res.push_field(nf);
                    }
                }
                j += 1;
            }
        }
    }
    if let Some(r) = record {
        res.char_loc = r.char_loc;
    }
    res
}

fn uniq(r: &mut Record) {
    let fields: Vec<(String, String)> = r.fields().map(|f| (f.name.clone(), f.value.clone())).collect();
    let n = fields.len();
    let mut remove = vec![false; n];
    for i in 0..n {
        if !remove[i] {
            for j in 0..n {
                if j != i && fields[i] == fields[j] {
                    remove[j] = true;
                }
            }
        }
    }
    let mut k = 0;
    r.elems.retain(|e| match e {
        Elem::Field(_) => {
            let keep = !remove[k];
            k += 1;
            keep
        }
        _ => true,
    });
}

fn contains_value(r: &Record, s: &str, icase: bool) -> bool {
    r.fields().any(|f| {
        if icase {
            f.value.to_ascii_lowercase().contains(&s.to_ascii_lowercase())
        } else {
            f.value.contains(s)
        }
    })
}

pub fn run(db: &Db, q: &Query) -> Option<Rset> {
    let mut res = Rset::default();
    let mut rset: Rset = match db.get_by_type(q.rtype.as_deref()) {
        Some(i) => {
            let mut r = db.rsets[i].clone();
            if let (Some(j), Some(t)) = (&q.join, &q.rtype) {
                let props = Props::from_rset(&r);
                if let Some(crate::types::Kind::Rec(target)) = props.field_type(j) {
                    if db.get_by_type(Some(&target)).is_some() {
                        match join(db, t, j, &target) {
                            Some(jr) => r = jr,
                            None => return None,
                        }
                    }
                }
            }
            r
        }
        None => {
            if q.rtype.is_none() && db.rsets.len() == 1 {
                db.rsets[0].clone()
            } else {
                return Some(res);
            }
        }
    };
    if q.descriptor {
        res.descriptor = rset.descriptor.clone();
    }
    let all_calls = q.fex.as_ref().map_or(false, |f| f.iter().all(|e| e.function.is_some()));
    if q.fex.is_some() && q.group_by.is_none() && all_calls {
        let r = process_fex(&rset, None, &q.fex);
        res.elems.push(RElem::Record(r));
        return Some(res);
    }
    // rec_rset_sort re-reads the descriptor after sorting, which resets the
    // ordering to %sort when the descriptor has one.
    let mut props = Props::from_rset(&rset);
    let desc_sort = props.sort.clone();
    let mut order = desc_sort.clone();
    if let Some(g) = &q.group_by {
        order = Some(g.clone());
        sort_rset(&mut rset, &props, g);
        if desc_sort.is_some() {
            order = desc_sort.clone();
        }
        group_rset(&mut rset, &props, g);
    }
    if let Some(s) = &q.sort_by {
        order = Some(s.clone());
    }
    if let Some(o) = order {
        sort_rset(&mut rset, &props, &o);
    }
    props.sort = None;
    let mut num_rec: usize = 0;
    let records: Vec<Record> = rset.records().cloned().collect();
    for (i, record) in records.iter().enumerate() {
        num_rec = i;
        let selected = if let Some(qs) = &q.quick {
            contains_value(record, qs, q.icase)
        } else if let Some(sex) = q.sex {
            sex.matches(record)
        } else if let Some(idx) = &q.index {
            idx.iter().any(|(min, max)| match max {
                None => num_rec == *min,
                Some(mx) => num_rec >= *min && num_rec <= *mx,
            })
        } else {
            true
        };
        if !selected {
            continue;
        }
        let mut out = process_fex(&rset, Some(record), &q.fex);
        if out.elems.is_empty() {
            continue;
        }
        if q.uniq {
            uniq(&mut out);
        }
        res.elems.push(RElem::Record(out));
    }
    let _ = num_rec;
    Some(res)
}
