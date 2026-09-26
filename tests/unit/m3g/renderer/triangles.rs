use super::covered_span;

#[test]
fn row_spans_match_individual_top_left_edge_tests() {
    let mut random = 0x1843_2026_u32;
    let mut next = || {
        random ^= random << 13;
        random ^= random >> 17;
        random ^= random << 5;
        random
    };
    // Include horizontal edges, exact edge hits, either winding, empty rows,
    // and coefficients near the rasterizer's coordinate guard range.
    for sample in 0..20_000 {
        let scale = if sample % 2 == 0 { 1 } else { 1_i64 << 40 };
        let edges = std::array::from_fn(|_| (i64::from(next() % 1025) - 512) * scale);
        let steps = std::array::from_fn(|_| (i64::from(next() % 65) - 32) * scale);
        let top_left = std::array::from_fn(|_| next() & 1 != 0);
        let width = (next() % 65) as i32;
        let expected: Vec<_> = (0..width)
            .filter(|x| {
                (0..3).all(|index| {
                    let edge = edges[index] + i64::from(*x) * steps[index];
                    edge > 0 || edge == 0 && top_left[index]
                })
            })
            .collect();
        assert_eq!(
            covered_span(edges, steps, top_left, width).collect::<Vec<_>>(),
            expected,
            "edges={edges:?} steps={steps:?} top_left={top_left:?} width={width}",
        );
    }
}
