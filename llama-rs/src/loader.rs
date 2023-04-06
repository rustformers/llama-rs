use std::{fs::File, io::BufReader};

use crate::*;

pub(crate) fn read_bytes<const N: usize>(reader: &mut impl BufRead) -> Result<[u8; N], LoadError> {
    let mut bytes = [0u8; N];
    reader
        .read_exact(&mut bytes)
        .map_err(|e| LoadError::ReadExactFailed {
            source: e,
            bytes: N,
        })?;
    Ok(bytes)
}

pub(crate) fn read_i32(reader: &mut impl BufRead) -> Result<i32, LoadError> {
    Ok(i32::from_le_bytes(read_bytes::<4>(reader)?))
}

pub(crate) fn read_u32(reader: &mut impl BufRead) -> Result<u32, LoadError> {
    Ok(u32::from_le_bytes(read_bytes::<4>(reader)?))
}

pub(crate) fn read_f32(reader: &mut impl BufRead) -> Result<f32, LoadError> {
    Ok(f32::from_le_bytes(read_bytes::<4>(reader)?))
}

/// Helper function. Reads a string from the buffer and returns it.
pub(crate) fn read_string(reader: &mut BufReader<File>, len: usize) -> Result<String, LoadError> {
    let mut buf = vec![0; len];
    reader
        .read_exact(&mut buf)
        .map_err(|e| LoadError::ReadExactFailed {
            source: e,
            bytes: buf.len(),
        })?;
    let s = String::from_utf8(buf)?;
    Ok(s)
}

#[derive(PartialEq)]
pub(crate) enum ModelType {
    GGMF,
    GGJT,
    Unversioned,
}

pub(crate) fn load_weights_ggmf_or_unversioned(
    mut reader: std::io::BufReader<std::fs::File>,
    main_path: &Path,
    load_progress_callback: impl Fn(LoadProgress),
    model: &Model,
) -> Result<(), LoadError> {
    let file_offset = reader.stream_position()?;
    drop(reader);

    let paths = util::find_all_model_files(main_path)?;

    let n_parts = paths.len();
    Ok(for (i, part_path) in paths.into_iter().enumerate() {
        let part_id = i;

        load_progress_callback(LoadProgress::PartLoading {
            file: &part_path,
            current_part: i,
            total_parts: n_parts,
        });

        let mut part_reader = BufReader::new(File::open(&part_path)?);

        // Skip metadata
        part_reader.seek(SeekFrom::Start(file_offset))?;

        let mut total_size = 0;
        let mut n_tensors = 0;

        // Load weights
        loop {
            // NOTE: Implementation from #![feature(buf_read_has_data_left)]
            let is_eof = part_reader.fill_buf().map(|b| b.is_empty())?;

            if is_eof {
                break;
            }

            let n_dims = usize::try_from(read_i32(&mut part_reader)?)?;
            let length = read_i32(&mut part_reader)?;
            let ftype = read_u32(&mut part_reader)?;

            let mut nelements = 1;
            let mut ne = [1i64, 1i64];

            #[allow(clippy::needless_range_loop)]
            for i in 0..n_dims {
                ne[i] = read_i32(&mut part_reader)? as i64;
                nelements *= usize::try_from(ne[i])?;
            }

            let tensor_name = read_string(&mut part_reader, length as usize)?;

            let Some(tensor) = model.tensors.get(&tensor_name)
                else {
                    return Err(LoadError::UnknownTensor { tensor_name, path: part_path });
                };

            // split_type = 0: split by columns
            // split_type = 1: split by rows
            //
            // split_type = 0:
            // regex:
            //   - tok_embeddings.*
            //   - layers.*.attention.wo.weight
            //   - layers.*.feed_forward.w2.weight

            // split_type = 1:
            // regex:
            //   - output.*
            //   - layers.*.attention.wq.weight
            //   - layers.*.attention.wk.weight
            //   - layers.*.attention.wv.weight
            //   - layers.*.feed_forward.w1.weight
            //   - layers.*.feed_forward.w3.weight
            #[allow(clippy::if_same_then_else)]
            let split_type = if tensor_name.contains("tok_embeddings") {
                0
            } else if tensor_name.contains("layers") {
                if tensor_name.contains("attention.wo.weight") {
                    0
                } else if tensor_name.contains("feed_forward.w2.weight") {
                    0
                } else {
                    1
                }
            } else if tensor_name.contains("output") {
                1
            } else {
                0
            };

            if n_dims == 1 {
                if tensor.nelements() != nelements {
                    return Err(LoadError::TensorWrongSize {
                        tensor_name,
                        path: part_path,
                    });
                }
            } else if tensor.nelements() / n_parts != nelements {
                return Err(LoadError::TensorWrongSize {
                    tensor_name,
                    path: part_path,
                });
            }

            if n_dims == 1 {
                if tensor.get_ne()[0] != ne[0] || tensor.get_ne()[1] != ne[1] {
                    return Err(LoadError::TensorWrongSize {
                        tensor_name,
                        path: part_path,
                    });
                }
            } else if split_type == 0 {
                if tensor.get_ne()[0] / i64::try_from(n_parts)? != ne[0]
                    || tensor.get_ne()[1] != ne[1]
                {
                    return Err(LoadError::TensorWrongSize {
                        tensor_name,
                        path: part_path,
                    });
                }
            } else if tensor.get_ne()[0] != ne[0]
                || tensor.get_ne()[1] / i64::try_from(n_parts)? != ne[1]
            {
                return Err(LoadError::TensorWrongSize {
                    tensor_name,
                    path: part_path,
                });
            }

            let bpe = match ftype {
                0 => ggml::type_size(ggml::Type::F32),
                1 => ggml::type_size(ggml::Type::F16),
                2 => {
                    assert_eq!(ne[0] % 64, 0);
                    ggml::type_size(ggml::Type::Q4_0)
                }
                3 => {
                    assert_eq!(ne[0] % 64, 0);
                    ggml::type_size(ggml::Type::Q4_1)
                }
                _ => {
                    return Err(LoadError::InvalidFtype {
                        tensor_name,
                        ftype,
                        path: part_path,
                    })
                }
            };

            if n_dims == 1 || n_parts == 1 {
                if (nelements * bpe) / ggml::blck_size(tensor.get_type()) != tensor.nbytes() {
                    return Err(LoadError::TensorWrongSize {
                        tensor_name,
                        path: part_path,
                    });
                }

                if part_id == 0 {
                    // SAFETY: yolo, same as original code
                    let slice = unsafe {
                        let data = tensor.data();
                        std::slice::from_raw_parts_mut(data as *mut u8, tensor.nbytes())
                    };
                    part_reader.read_exact(slice)?;
                } else {
                    part_reader.seek(SeekFrom::Current(tensor.nbytes() as i64))?;
                }

                total_size += tensor.nbytes();
            } else {
                if (nelements * bpe) / ggml::blck_size(tensor.get_type())
                    != tensor.nbytes() / n_parts
                {
                    return Err(LoadError::TensorWrongSize {
                        tensor_name,
                        path: part_path,
                    });
                }

                if split_type == 0 {
                    let np0 = ne[0];
                    let row_size = (usize::try_from(tensor.get_ne()[0])?
                        / ggml::blck_size(tensor.get_type()))
                        * ggml::type_size(tensor.get_type());

                    assert_eq!(row_size, tensor.get_nb()[1]);

                    for i1 in 0..ne[1] {
                        let offset_row = i1 as usize * row_size;
                        let offset = offset_row
                            + ((part_id * np0 as usize) / ggml::blck_size(tensor.get_type()))
                                * ggml::type_size(tensor.get_type());
                        // SAFETY: yolo, same as original code
                        unsafe {
                            let ptr = tensor.data().add(offset);
                            let slice =
                                std::slice::from_raw_parts_mut(ptr as *mut u8, row_size / n_parts);
                            part_reader.read_exact(slice)?;
                        }
                    }
                } else {
                    let np1 = ne[1];
                    let row_size = (usize::try_from(tensor.get_ne()[0])?
                        / ggml::blck_size(tensor.get_type()))
                        * ggml::type_size(tensor.get_type());

                    for i1 in 0..ne[1] {
                        let offset_row = (i1 as usize + part_id * np1 as usize) * row_size;
                        // SAFETY: yolo, same as original code
                        unsafe {
                            let ptr = tensor.data().add(offset_row);
                            let slice = std::slice::from_raw_parts_mut(ptr as *mut u8, row_size);
                            part_reader.read_exact(slice)?;
                        }
                    }
                }

                total_size += tensor.nbytes() / n_parts;
            }

            n_tensors += 1;
            load_progress_callback(LoadProgress::PartTensorLoaded {
                file: &part_path,
                current_tensor: n_tensors.try_into()?,
                tensor_count: model.tensors.len(),
            });
        }

        load_progress_callback(LoadProgress::PartLoaded {
            file: &part_path,
            byte_size: total_size,
            tensor_count: n_tensors.try_into()?,
        });
    })
}

pub(crate) fn load_weights_ggjt(
    mut reader: std::io::BufReader<std::fs::File>,
    main_path: &Path,
    load_progress_callback: impl Fn(LoadProgress),
    model: &Model,
) -> Result<(), LoadError> {
    todo!("GGJT load weights");
}
