//! libc++'s `std::sort` (`__algorithm/sort.h`, the same from LLVM 17 to 21),
//! which `Skyline::internal_build_skyline` sorts its buildings with. The sort
//! is not stable, and buildings that start at one point at one height are
//! equivalent; which of them comes first changes the skyline LilyPond builds,
//! so the order must be libc++'s, that of Homebrew's LilyPond for macOS.
//! libstdc++'s introsort may order such ties differently.
//!
//! For a comparator other than `std::less`, libc++ sorts without its
//! branchless and bitset variants: introsort, with insertion sort below 24
//! elements, Tukey's ninther above 128, pdqsort's check for an already
//! partitioned range and heap sort past the depth limit.

/// `std::sort(v.begin(), v.end(), less)`. Layout sorts with it too, where
/// LilyPond's `std::sort` may tie (MS3 step 27).
pub fn sort<T: Copy>(v: &mut [T], less: &impl Fn(&T, &T) -> bool) {
    let n = v.len();
    let depth = if n == 0 {
        0
    } else {
        2 * (usize::BITS - 1 - n.leading_zeros()) as usize
    };
    introsort(v, 0, n, less, depth, true);
}

fn sort3<T>(v: &mut [T], x: usize, y: usize, z: usize, less: &impl Fn(&T, &T) -> bool) {
    if !less(&v[y], &v[x]) {
        if !less(&v[z], &v[y]) {
            return;
        }
        v.swap(y, z);
        if less(&v[y], &v[x]) {
            v.swap(x, y);
        }
        return;
    }
    if less(&v[z], &v[y]) {
        v.swap(x, z);
        return;
    }
    v.swap(x, y);
    if less(&v[z], &v[y]) {
        v.swap(y, z);
    }
}

fn sort4<T>(v: &mut [T], x: [usize; 4], less: &impl Fn(&T, &T) -> bool) {
    sort3(v, x[0], x[1], x[2], less);
    for i in (0..3).rev() {
        if !less(&v[x[i + 1]], &v[x[i]]) {
            break;
        }
        v.swap(x[i], x[i + 1]);
    }
}

fn sort5<T>(v: &mut [T], x: [usize; 5], less: &impl Fn(&T, &T) -> bool) {
    sort4(v, [x[0], x[1], x[2], x[3]], less);
    for i in (0..4).rev() {
        if !less(&v[x[i + 1]], &v[x[i]]) {
            break;
        }
        v.swap(x[i], x[i + 1]);
    }
}

/// The cases of up to five elements; false for a longer range.
fn sort_few<T>(v: &mut [T], first: usize, last: usize, less: &impl Fn(&T, &T) -> bool) -> bool {
    let f = first;
    match last - first {
        0 | 1 => {}
        2 => {
            if less(&v[f + 1], &v[f]) {
                v.swap(f, f + 1);
            }
        }
        3 => sort3(v, f, f + 1, f + 2, less),
        4 => sort4(v, [f, f + 1, f + 2, f + 3], less),
        5 => sort5(v, [f, f + 1, f + 2, f + 3, f + 4], less),
        _ => return false,
    }
    true
}

/// `__insertion_sort`, or with `guarded` false `__insertion_sort_unguarded`,
/// which relies on `v[first - 1]` being no greater than any of the range.
fn insertion_sort<T: Copy>(
    v: &mut [T],
    first: usize,
    last: usize,
    less: &impl Fn(&T, &T) -> bool,
    guarded: bool,
) {
    for i in first + 1..last {
        if less(&v[i], &v[i - 1]) {
            let t = v[i];
            let mut j = i;
            let mut k = i - 1;
            loop {
                v[j] = v[k];
                j = k;
                if guarded && j == first {
                    break;
                }
                k -= 1;
                if !less(&t, &v[k]) {
                    break;
                }
            }
            v[j] = t;
        }
    }
}

/// `__insertion_sort_incomplete`: gives up after moving 8 elements; true if
/// the range is sorted.
fn insertion_sort_incomplete<T: Copy>(
    v: &mut [T],
    first: usize,
    last: usize,
    less: &impl Fn(&T, &T) -> bool,
) -> bool {
    if sort_few(v, first, last, less) {
        return true;
    }
    let mut j = first + 2;
    sort3(v, first, first + 1, j, less);
    let mut count = 0;
    for i in j + 1..last {
        if less(&v[i], &v[j]) {
            let t = v[i];
            let mut k = j;
            j = i;
            loop {
                v[j] = v[k];
                j = k;
                if j == first {
                    break;
                }
                k -= 1;
                if !less(&t, &v[k]) {
                    break;
                }
            }
            v[j] = t;
            count += 1;
            if count == 8 {
                return i + 1 == last;
            }
        }
        j = i;
    }
    true
}

/// `__partition_with_equals_on_right` around the pivot `v[first]`: its
/// position, and whether the range was already partitioned.
fn partition_equals_on_right<T: Copy>(
    v: &mut [T],
    mut first: usize,
    mut last: usize,
    less: &impl Fn(&T, &T) -> bool,
) -> (usize, bool) {
    let begin = first;
    let pivot = v[first];
    loop {
        first += 1;
        if !less(&v[first], &pivot) {
            break;
        }
    }
    if begin == first - 1 {
        while first < last {
            last -= 1;
            if less(&v[last], &pivot) {
                break;
            }
        }
    } else {
        loop {
            last -= 1;
            if less(&v[last], &pivot) {
                break;
            }
        }
    }
    let already_partitioned = first >= last;
    while first < last {
        v.swap(first, last);
        loop {
            first += 1;
            if !less(&v[first], &pivot) {
                break;
            }
        }
        loop {
            last -= 1;
            if less(&v[last], &pivot) {
                break;
            }
        }
    }
    let pivot_pos = first - 1;
    v[begin] = v[pivot_pos];
    v[pivot_pos] = pivot;
    (pivot_pos, already_partitioned)
}

/// `__partition_with_equals_on_left`: the elements equivalent to the pivot
/// go left of it; returns the start of the rest.
fn partition_equals_on_left<T: Copy>(
    v: &mut [T],
    mut first: usize,
    mut last: usize,
    less: &impl Fn(&T, &T) -> bool,
) -> usize {
    let begin = first;
    let pivot = v[first];
    if less(&pivot, &v[last - 1]) {
        loop {
            first += 1;
            if less(&pivot, &v[first]) {
                break;
            }
        }
    } else {
        loop {
            first += 1;
            if !(first < last && !less(&pivot, &v[first])) {
                break;
            }
        }
    }
    if first < last {
        loop {
            last -= 1;
            if !less(&pivot, &v[last]) {
                break;
            }
        }
    }
    while first < last {
        v.swap(first, last);
        loop {
            first += 1;
            if less(&pivot, &v[first]) {
                break;
            }
        }
        loop {
            last -= 1;
            if !less(&pivot, &v[last]) {
                break;
            }
        }
    }
    let pivot_pos = first - 1;
    v[begin] = v[pivot_pos];
    v[pivot_pos] = pivot;
    first
}

fn introsort<T: Copy>(
    v: &mut [T],
    mut first: usize,
    mut last: usize,
    less: &impl Fn(&T, &T) -> bool,
    mut depth: usize,
    mut leftmost: bool,
) {
    const LIMIT: usize = 24;
    const NINTHER_THRESHOLD: usize = 128;
    loop {
        let len = last - first;
        if sort_few(v, first, last, less) {
            return;
        }
        if len < LIMIT {
            insertion_sort(v, first, last, less, leftmost);
            return;
        }
        if depth == 0 {
            heap_sort(&mut v[first..last], less);
            return;
        }
        depth -= 1;
        let half = len / 2;
        if len > NINTHER_THRESHOLD {
            sort3(v, first, first + half, last - 1, less);
            sort3(v, first + 1, first + half - 1, last - 2, less);
            sort3(v, first + 2, first + half + 1, last - 3, less);
            sort3(v, first + half - 1, first + half, first + half + 1, less);
            v.swap(first, first + half);
        } else {
            sort3(v, first + half, first, last - 1, less);
        }
        if !leftmost && !less(&v[first - 1], &v[first]) {
            first = partition_equals_on_left(v, first, last, less);
            continue;
        }
        let (i, already_partitioned) = partition_equals_on_right(v, first, last, less);
        if already_partitioned {
            let left_sorted = insertion_sort_incomplete(v, first, i, less);
            if insertion_sort_incomplete(v, i + 1, last, less) {
                if left_sorted {
                    return;
                }
                last = i;
                continue;
            } else if left_sorted {
                first = i + 1;
                continue;
            }
        }
        introsort(v, first, i, less, depth, leftmost);
        leftmost = false;
        first = i + 1;
    }
}

/// `__partial_sort` of the whole range: `__make_heap`, then `__sort_heap`
/// with Floyd's `__pop_heap`.
fn heap_sort<T: Copy>(v: &mut [T], less: &impl Fn(&T, &T) -> bool) {
    let n = v.len();
    if n > 1 {
        for start in (0..=(n - 2) / 2).rev() {
            sift_down(v, n, start, less);
        }
    }
    for len in (2..=n).rev() {
        let top = v[0];
        let mut hole = floyd_sift_down(v, len, less);
        let last = len - 1;
        if hole == last {
            v[hole] = top;
        } else {
            v[hole] = v[last];
            hole += 1;
            v[last] = top;
            sift_up(v, hole, less);
        }
    }
}

fn sift_down<T: Copy>(v: &mut [T], len: usize, mut start: usize, less: &impl Fn(&T, &T) -> bool) {
    let mut child = start;
    if len < 2 || (len - 2) / 2 < child {
        return;
    }
    child = 2 * child + 1;
    if child + 1 < len && less(&v[child], &v[child + 1]) {
        child += 1;
    }
    if less(&v[child], &v[start]) {
        return;
    }
    let top = v[start];
    loop {
        v[start] = v[child];
        start = child;
        if (len - 2) / 2 < child {
            break;
        }
        child = 2 * child + 1;
        if child + 1 < len && less(&v[child], &v[child + 1]) {
            child += 1;
        }
        if less(&v[child], &top) {
            break;
        }
    }
    v[start] = top;
}

fn floyd_sift_down<T: Copy>(v: &mut [T], len: usize, less: &impl Fn(&T, &T) -> bool) -> usize {
    let mut hole = 0;
    let mut child = 0;
    loop {
        child = 2 * child + 1;
        if child + 1 < len && less(&v[child], &v[child + 1]) {
            child += 1;
        }
        v[hole] = v[child];
        hole = child;
        if child > (len - 2) / 2 {
            return hole;
        }
    }
}

/// `__sift_up` of `v[len - 1]`.
fn sift_up<T: Copy>(v: &mut [T], len: usize, less: &impl Fn(&T, &T) -> bool) {
    if len <= 1 {
        return;
    }
    let mut last = len - 1;
    let mut parent = (len - 2) / 2;
    if less(&v[parent], &v[last]) {
        let t = v[last];
        loop {
            v[last] = v[parent];
            last = parent;
            if parent == 0 {
                break;
            }
            parent = (parent - 1) / 2;
            if !less(&v[parent], &t) {
                break;
            }
        }
        v[last] = t;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    /// Sorts pairs by their first number; the second shows where equivalent
    /// ones end up.
    fn sorted(v: &[(u32, u32)], depth: Option<usize>) -> Vec<(u32, u32)> {
        let mut v = v.to_vec();
        let less = |a: &(u32, u32), b: &(u32, u32)| a.0 < b.0;
        match depth {
            None => sort(&mut v, &less),
            Some(depth) => {
                let n = v.len();
                introsort(&mut v, 0, n, &less, depth, true)
            }
        }
        v
    }

    /// `n` pairs of a key below `keys` and their index, from a xorshift
    /// generator.
    fn pairs(n: u32, keys: u32) -> Vec<(u32, u32)> {
        let mut x = 0x2545_f491_u32 ^ n.wrapping_mul(2_654_435_761) ^ keys;
        (0..n)
            .map(|i| {
                x ^= x << 13;
                x ^= x >> 17;
                x ^= x << 5;
                (x % keys, i)
            })
            .collect()
    }

    #[test]
    fn every_length_and_the_heap_sort_order_by_the_key() {
        for n in 0..300 {
            for depth in [None, Some(0), Some(1)] {
                let v = sorted(&pairs(n, 7), depth);
                assert!(v.windows(2).all(|w| w[0].0 <= w[1].0), "{n} {depth:?}");
                let mut seen: Vec<u32> = v.iter().map(|p| p.1).collect();
                seen.sort_unstable();
                assert!(seen.iter().copied().eq(0..n));
            }
        }
    }

    /// The order of ties that libc++ gives (`std::sort` of `std::pair<unsigned,
    /// unsigned>` by `first`, Apple clang 17): partitioned, not stable.
    #[test]
    fn ties_come_out_in_libcxx_s_order() {
        let v = sorted(&pairs(40, 3), None);
        let order: Vec<u32> = v.iter().map(|p| p.1).collect();
        assert_eq!(
            order,
            [
                16, 37, 2, 3, 36, 29, 6, 22, 8, 9, 10, 11, 25, 18, 33, 31, 34, 32, 13, 19, 39, 21,
                7, 5, 28, 20, 26, 27, 24, 23, 30, 17, 15, 14, 12, 35, 4, 1, 38, 0
            ]
        );
    }
}
