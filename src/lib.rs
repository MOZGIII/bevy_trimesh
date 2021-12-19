#![doc = include_str!("../README.md")]
#![feature(array_chunks)]

use bevy::{
    prelude::*,
    render::mesh::{Indices, VertexAttributeValues},
};
use parry3d::math::{Point, Real};
pub use parry3d::{self, shape::TriMesh};

/// The geometry extraction error.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ExtractGeometryError {
    /// Sometimes meshes come without vertex data.
    #[error("no vertex position data found in the specified mesh")]
    NoVertexPositionData,
    /// Sometimes meshes come without indicies.
    #[error("no vertex indicies found in the specified mesh")]
    NoVertexIndicies,
}

/// Extract the geometry from a bevy [`Mesh`].
pub fn extract_geometry(
    mesh: &Mesh,
) -> Result<(&VertexAttributeValues, &Indices), ExtractGeometryError> {
    let verticies = mesh
        .attribute(Mesh::ATTRIBUTE_POSITION)
        .ok_or(ExtractGeometryError::NoVertexPositionData)?;
    let indicies = mesh
        .indices()
        .ok_or(ExtractGeometryError::NoVertexIndicies)?;
    Ok((verticies, indicies))
}

/// An error indicating the format is not supported by this crate.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[error("unsupported format: {0}")]
pub struct UnsupportedFormatError(&'static str);

/// Convert vertices from the bevy format to a format that trimesh ingests.
pub fn convert_verticies(
    verticies: &VertexAttributeValues,
) -> Result<impl Iterator<Item = Point<Real>> + '_, UnsupportedFormatError> {
    let verticies = match verticies {
        VertexAttributeValues::Float3(val) => val,
        _ => return Err(UnsupportedFormatError("only [f32; 3] is supported")),
    };
    Ok(verticies.iter().map(|vertex| Point::from_slice(vertex)))
}

/// Convert indicies from the bevy format to a format that trimesh ingests.
pub fn convert_indicies(
    indicies: &Indices,
) -> Result<impl Iterator<Item = [u32; 3]> + '_, UnsupportedFormatError> {
    let indicies = match indicies {
        Indices::U32(ref val) => val,
        _ => return Err(UnsupportedFormatError("only u32 is supported")),
    };
    Ok(indicies.array_chunks().copied())
}

/// The an error while building the [`TriMesh`] geometry from a [`Mesh`].
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum TriMeshBuildError {
    /// Extraction failed.
    #[error(transparent)]
    Extracion(#[from] ExtractGeometryError),
    /// Verticies conversion failed.
    #[error("verticies: {0}")]
    UnsupportedVerexDataFormat(#[source] UnsupportedFormatError),
    /// Indicies conversion failed.
    #[error("indicies: {0}")]
    UnsupportedIndexFormat(#[source] UnsupportedFormatError),
}

/// Prepare the inputs to the [`TriMesh`] constructor from the [`Mesh`]
/// geometry in the form of iterators directly over the mesh geometry.
///
/// The use of iterators instead of copied [`Vec`]s allows adding a layer of
/// transformations over the output data before doing a `collect` to populate
/// the cache.
pub fn prepare_trimesh_from_mesh(
    mesh: &Mesh,
) -> Result<
    (
        impl Iterator<Item = Point<Real>> + '_,
        impl Iterator<Item = [u32; 3]> + '_,
    ),
    TriMeshBuildError,
> {
    let (verticies, indicies) = extract_geometry(mesh)?;
    let verticies =
        convert_verticies(verticies).map_err(TriMeshBuildError::UnsupportedVerexDataFormat)?;
    let indicies = convert_indicies(indicies).map_err(TriMeshBuildError::UnsupportedIndexFormat)?;
    Ok((verticies, indicies))
}

/// Create a [`TriMesh`] from the [`Mesh`] geometry.
///
/// You might want to use raw [`prepare_trimesh_from_mesh`] and call
/// [`TriMesh::new`] yourself if you need to advanced caching/translation of
/// the verticies and/or indicies.
pub fn trimesh_from_mesh(mesh: &Mesh) -> Result<TriMesh, TriMeshBuildError> {
    let (verticies, indicies) = prepare_trimesh_from_mesh(mesh)?;
    let trimesh = TriMesh::new(verticies.collect(), indicies.collect());
    Ok(trimesh)
}

/// Holds the [`TriMesh`] geometry.
pub struct CachedTriMeshBuilder {
    /// Precomputed verticies to use when constructing a [`TriMesh`].
    pub verticies: Vec<Point<Real>>,
    /// Precomputed indicies to use when constructing a [`TriMesh`].
    pub indicies: Vec<[u32; 3]>,
}

impl CachedTriMeshBuilder {
    /// Extract the geometry from a [`Mesh`] and create
    /// a [`CachedTriMeshBuilder`].
    pub fn from_mesh(mesh: &Mesh) -> Result<Self, TriMeshBuildError> {
        let (verticies, indicies) = prepare_trimesh_from_mesh(mesh)?;

        // Cache the geometry to we reuse the buffer when spawning walls.
        let verticies: Vec<_> = verticies.collect();
        let indicies: Vec<_> = indicies.collect();

        Ok(Self {
            verticies,
            indicies,
        })
    }

    /// Build a new [`TriMesh`] from the precomputed geometry.
    ///
    /// To be used multiple times to leverage the cached data.
    pub fn build(&self) -> TriMesh {
        TriMesh::new(self.verticies.clone(), self.indicies.clone())
    }

    /// Build a new [`TriMesh`] from the precomputed geometry, while applying
    /// a given [`transform`] to each vertex.
    ///
    /// To be used multiple times to leverage the cached data.
    pub fn build_with_vertex_transform(
        &self,
        transform: impl Fn(Point<Real>) -> Point<Real>,
    ) -> TriMesh {
        let verticies = self.verticies.iter().copied().map(transform).collect();
        TriMesh::new(verticies, self.indicies.clone())
    }
}
