//! Submatrix (SMX) ↔ sequential matrix conversion.
//!
//! Ported from `smxutil.c`. JEOL Delta stores multi-dimensional data in a
//! "submatrix" tiled layout for efficient random access. This module converts
//! between SMX and sequential (row-major) order.

/// Maximum number of dimensions for SMX operations.
pub const SMX_MAXDIM: usize = 8;

/// Error codes for SMX operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmxError {
    MaxDim,
    NullSize,
    MatLimit,
    SmxLimit,
    EdgeMismatch,
    SizeMismatch,
}

/// Precomputed state for SMX ↔ matrix conversion.
struct SmxState {
    smx_size: [i32; SMX_MAXDIM],
    smx_edge: [i32; SMX_MAXDIM],
    smx_jump: [i64; SMX_MAXDIM],
    smx_x1: [i32; SMX_MAXDIM],
    smx_xn: [i32; SMX_MAXDIM],

    mat_size: [i32; SMX_MAXDIM],
    mat_jump: [i64; SMX_MAXDIM],
    mat_x1: [i32; SMX_MAXDIM],
    mat_xn: [i32; SMX_MAXDIM],

    smx_blocks: [i32; SMX_MAXDIM],
    block_jump: [i64; SMX_MAXDIM],
    block_size: i64,

    dim_count: usize,
    word_size: i32,
}

impl SmxState {
    fn new(
        mat_size_in: &[i32],
        mat_x1_in: Option<&[i32]>,
        mat_xn_in: Option<&[i32]>,
        smx_size_in: &[i32],
        smx_x1_in: Option<&[i32]>,
        smx_xn_in: Option<&[i32]>,
        edge_in: &[i32],
        word_size: i32,
        dim_count: usize,
    ) -> Result<Self, SmxError> {
        if dim_count > SMX_MAXDIM {
            return Err(SmxError::MaxDim);
        }
        if dim_count == 0 {
            return Err(SmxError::NullSize);
        }

        let mut state = SmxState {
            smx_size: [0; SMX_MAXDIM],
            smx_edge: [0; SMX_MAXDIM],
            smx_jump: [0; SMX_MAXDIM],
            smx_x1: [0; SMX_MAXDIM],
            smx_xn: [0; SMX_MAXDIM],
            mat_size: [0; SMX_MAXDIM],
            mat_jump: [0; SMX_MAXDIM],
            mat_x1: [0; SMX_MAXDIM],
            mat_xn: [0; SMX_MAXDIM],
            smx_blocks: [0; SMX_MAXDIM],
            block_jump: [0; SMX_MAXDIM],
            block_size: 0,
            dim_count,
            word_size,
        };

        for i in 0..dim_count {
            state.smx_size[i] = smx_size_in[i];
            state.mat_size[i] = mat_size_in[i];
            state.smx_edge[i] = edge_in[i];

            state.mat_x1[i] = mat_x1_in.map_or(1, |v| v[i]);
            state.smx_x1[i] = smx_x1_in.map_or(1, |v| v[i]);
            state.mat_xn[i] = mat_xn_in.map_or(state.mat_size[i], |v| v[i]);
            state.smx_xn[i] = smx_xn_in.map_or(state.smx_size[i], |v| v[i]);

            if state.smx_size[i] < 1 || state.mat_size[i] < 1 || state.smx_edge[i] < 1 {
                return Err(SmxError::NullSize);
            }

            // Ensure limits are ordered
            if state.mat_x1[i] > state.mat_xn[i] {
                let t = state.mat_x1[i];
                state.mat_x1[i] = state.mat_xn[i];
                state.mat_xn[i] = t;
            }
            if state.smx_x1[i] > state.smx_xn[i] {
                let t = state.smx_x1[i];
                state.smx_x1[i] = state.smx_xn[i];
                state.smx_xn[i] = t;
            }

            if state.mat_x1[i] < 1 || state.mat_xn[i] > state.mat_size[i] {
                return Err(SmxError::MatLimit);
            }
            if state.smx_x1[i] < 1 || state.smx_xn[i] > state.smx_size[i] {
                return Err(SmxError::SmxLimit);
            }
            if state.smx_size[i] % state.smx_edge[i] != 0 {
                return Err(SmxError::EdgeMismatch);
            }
            if state.smx_xn[i] - state.smx_x1[i] != state.mat_xn[i] - state.mat_x1[i] {
                return Err(SmxError::SizeMismatch);
            }
        }

        state.block_size = word_size as i64;
        for i in 0..dim_count {
            state.smx_blocks[i] = state.smx_size[i] / state.smx_edge[i];
            state.block_size *= state.smx_edge[i] as i64;
        }

        // Compute jump tables
        for i in (0..dim_count).rev() {
            state.mat_jump[i] = word_size as i64;
            state.smx_jump[i] = word_size as i64;
            state.block_jump[i] = state.block_size;

            for j in (0..i).rev() {
                state.mat_jump[i] *= state.mat_size[j] as i64;
                state.smx_jump[i] *= state.smx_edge[j] as i64;
                state.block_jump[i] *= state.smx_blocks[j] as i64;
            }
        }

        Ok(state)
    }

    /// Get byte offset of a coordinate in the sequential matrix.
    fn get_mat_loc(&self, coords: &[i32]) -> i64 {
        let mut loc: i64 = 0;
        for i in 0..self.dim_count {
            loc += (coords[i] - 1) as i64 * self.mat_jump[i];
        }
        loc
    }

    /// Get byte offset of a coordinate in the submatrix layout.
    fn get_smx_loc(&self, coords: &[i32]) -> i64 {
        let mut loc: i64 = 0;
        for i in 0..self.dim_count {
            let n_div = (coords[i] - 1) / self.smx_edge[i];
            let n_mod = (coords[i] - 1) % self.smx_edge[i];
            loc += n_div as i64 * self.block_jump[i] + n_mod as i64 * self.smx_jump[i];
        }
        loc
    }
}

/// Convert submatrix-layout data to sequential (row-major) matrix layout.
///
/// # Arguments
/// - `smx_data`: Input data in submatrix layout.
/// - `mat_data`: Output buffer for sequential data.
/// - `mat_size`: Size of each dimension in the output matrix.
/// - `mat_x1`: Optional start limits for matrix (1-based). `None` = start at 1.
/// - `mat_xn`: Optional end limits for matrix. `None` = full size.
/// - `smx_size`: Size of each dimension in the submatrix data.
/// - `smx_x1`: Optional start limits for SMX. `None` = start at 1.
/// - `smx_xn`: Optional end limits for SMX. `None` = full size.
/// - `edge`: Submatrix tile edge sizes.
/// - `word_size`: Size of each data element in bytes.
/// - `dim_count`: Number of dimensions.
pub fn smx2matrix(
    smx_data: &[u8],
    mat_data: &mut [u8],
    mat_size: &[i32],
    mat_x1: Option<&[i32]>,
    mat_xn: Option<&[i32]>,
    smx_size: &[i32],
    smx_x1: Option<&[i32]>,
    smx_xn: Option<&[i32]>,
    edge: &[i32],
    word_size: i32,
    dim_count: usize,
) -> Result<(), SmxError> {
    let state = SmxState::new(
        mat_size, mat_x1, mat_xn, smx_size, smx_x1, smx_xn, edge, word_size, dim_count,
    )?;

    // Recursive conversion
    let mut src_loc = [0i32; SMX_MAXDIM];
    let mut dest_loc = [0i32; SMX_MAXDIM];
    smx2matrix_r(
        &state,
        smx_data,
        mat_data,
        dim_count,
        &mut src_loc,
        &mut dest_loc,
    );

    Ok(())
}

fn smx2matrix_r(
    state: &SmxState,
    smx_data: &[u8],
    mat_data: &mut [u8],
    dim: usize,
    src_loc: &mut [i32; SMX_MAXDIM],
    dest_loc: &mut [i32; SMX_MAXDIM],
) {
    let d = dim - 1;

    let mut coord = state.smx_x1[d];
    while coord <= state.smx_xn[d] {
        src_loc[d] = coord;
        dest_loc[d] = state.mat_x1[d] + coord - state.smx_x1[d];

        if d == 0 {
            let src_off = state.get_smx_loc(src_loc) as usize;
            let dest_off = state.get_mat_loc(dest_loc) as usize;
            let ws = state.word_size as usize;

            if src_off + ws <= smx_data.len() && dest_off + ws <= mat_data.len() {
                mat_data[dest_off..dest_off + ws].copy_from_slice(&smx_data[src_off..src_off + ws]);
            }
        } else {
            smx2matrix_r(state, smx_data, mat_data, d, src_loc, dest_loc);
        }

        coord += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_smx2matrix_1d() {
        // Simple 1D case: 8 float values in a submatrix with edge=4
        let word_size = 4;
        let dim_count = 1;
        let total = 8;
        let edge = [4];
        let mat_size = [total];
        let smx_size = [total];

        // SMX order with edge=4: [0,1,2,3, 4,5,6,7] (same as sequential for 1D)
        let mut smx_data = vec![0u8; (total * word_size) as usize];
        for i in 0..total as usize {
            let val = (i + 1) as f32;
            let bytes = val.to_ne_bytes();
            smx_data[i * 4..i * 4 + 4].copy_from_slice(&bytes);
        }

        let mut mat_data = vec![0u8; (total * word_size) as usize];

        smx2matrix(
            &smx_data,
            &mut mat_data,
            &mat_size,
            None,
            None,
            &smx_size,
            None,
            None,
            &edge,
            word_size,
            dim_count,
        )
        .unwrap();

        // In 1D with matching sizes, output should equal input
        for i in 0..total as usize {
            let val = f32::from_ne_bytes([
                mat_data[i * 4],
                mat_data[i * 4 + 1],
                mat_data[i * 4 + 2],
                mat_data[i * 4 + 3],
            ]);
            assert_eq!(val, (i + 1) as f32);
        }
    }
}
