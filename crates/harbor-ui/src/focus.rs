use harbor_platform::Key;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Focus {
    pub row: usize,
    pub col: usize,
}

pub fn reduce(focus: Focus, rows: &[usize], key: Key) -> Focus {
    if !has_items(rows) {
        return Focus { row: 0, col: 0 };
    }
    let mut f = clamp(focus, rows);
    match key {
        Key::Left => {
            let n = rows[f.row];
            f.col = (f.col + n - 1) % n;
        }
        Key::Right => {
            let n = rows[f.row];
            f.col = (f.col + 1) % n;
        }
        Key::Up => {
            if let Some(row) = prev_row(rows, f.row) {
                f.row = row;
                f.col = f.col.min(rows[row] - 1);
            }
        }
        Key::Down => {
            if let Some(row) = next_row(rows, f.row) {
                f.row = row;
                f.col = f.col.min(rows[row] - 1);
            }
        }
        _ => {}
    }
    f
}

fn has_items(rows: &[usize]) -> bool {
    rows.iter().any(|&n| n > 0)
}

fn clamp(focus: Focus, rows: &[usize]) -> Focus {
    if !has_items(rows) {
        return Focus { row: 0, col: 0 };
    }
    let mut row = focus.row.min(rows.len() - 1);
    if rows[row] == 0 {
        row = next_row(rows, row)
            .or_else(|| prev_row(rows, row))
            .unwrap_or(0);
    }
    let col = if rows[row] == 0 {
        0
    } else {
        focus.col.min(rows[row] - 1)
    };
    Focus { row, col }
}

fn next_row(rows: &[usize], from: usize) -> Option<usize> {
    rows.iter()
        .enumerate()
        .skip(from + 1)
        .find(|(_, n)| **n > 0)
        .map(|(i, _)| i)
}

fn prev_row(rows: &[usize], from: usize) -> Option<usize> {
    rows.iter()
        .enumerate()
        .take(from)
        .rev()
        .find(|(_, n)| **n > 0)
        .map(|(i, _)| i)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_state_stays_origin() {
        let empty: &[usize] = &[];
        let zeros = &[0, 0];
        for rows in [empty, zeros.as_slice()] {
            let f = reduce(Focus { row: 3, col: 2 }, rows, Key::Right);
            assert_eq!(f, Focus { row: 0, col: 0 });
            assert_eq!(reduce(f, rows, Key::Down), Focus { row: 0, col: 0 });
        }
    }

    #[test]
    fn shelf_wraps_horizontally() {
        let rows = &[4];
        let mut f = Focus { row: 0, col: 0 };
        f = reduce(f, rows, Key::Left);
        assert_eq!(f.col, 3);
        f = reduce(f, rows, Key::Right);
        assert_eq!(f.col, 0);
        f = reduce(f, rows, Key::Right);
        assert_eq!(f.col, 1);
    }

    #[test]
    fn up_down_moves_between_shelves() {
        let rows = &[3, 5, 2];
        let mut f = Focus { row: 0, col: 2 };
        f = reduce(f, rows, Key::Down);
        assert_eq!(f, Focus { row: 1, col: 2 });
        f = reduce(f, rows, Key::Down);
        assert_eq!(f, Focus { row: 2, col: 1 });
        f = reduce(f, rows, Key::Up);
        assert_eq!(f, Focus { row: 1, col: 1 });
    }

    #[test]
    fn skips_empty_row() {
        let rows = &[2, 0, 3];
        let mut f = Focus { row: 0, col: 0 };
        f = reduce(f, rows, Key::Down);
        assert_eq!(f, Focus { row: 2, col: 0 });
        f = reduce(f, rows, Key::Up);
        assert_eq!(f, Focus { row: 0, col: 0 });
    }

    #[test]
    fn ok_and_back_are_noops() {
        let rows = &[2, 2];
        let f = Focus { row: 1, col: 1 };
        assert_eq!(reduce(f, rows, Key::Ok), f);
        assert_eq!(reduce(f, rows, Key::Back), f);
    }
}
