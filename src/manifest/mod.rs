/*
 * Copyright (C) 2019 Josh Gao
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *      http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

use std::collections::{BTreeMap, HashMap};
use std::fs::File;
use std::io::BufReader;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Error};

use crate::tree::TreeConfig;
use crate::Config;

// The repo manifest format is described at
// https://gerrit.googlesource.com/git-repo/+/master/docs/manifest-format.md
#[derive(Debug, Deserialize)]
struct ManifestSchema {
  project: Vec<ProjectSchema>,
  remote: Vec<RemoteSchema>,
  default: Option<DefaultSchema>,
  include: Option<Vec<IncludeSchema>>,
}

/// Schema defined at https://gerrit.googlesource.com/git-repo/+/HEAD/docs/manifest-format.md#Element-remote
#[derive(Debug, Deserialize)]
struct RemoteSchema {
  #[serde(rename = "@name")]
  name: String,

  #[serde(rename = "@alias")]
  alias: Option<String>,

  #[serde(rename = "@fetch")]
  fetch: String,

  #[serde(rename = "@pushurl")]
  push_url: Option<String>,

  #[serde(rename = "@review")]
  review: Option<String>,

  #[serde(rename = "@revision")]
  revision: Option<String>,
}

/// Schema defined at https://gerrit.googlesource.com/git-repo/+/HEAD/docs/manifest-format.md#Element-default
#[derive(Debug, Deserialize)]
struct DefaultSchema {
  #[serde(rename = "@remote")]
  remote: Option<String>,

  #[serde(rename = "@revision")]
  revision: Option<String>,

  #[serde(rename = "@dest-branch")]
  dest_branch: Option<String>,

  #[serde(rename = "@upstream")]
  upstream: Option<String>,

  #[serde(rename = "@sync-j")]
  sync_j: Option<u32>,

  #[serde(rename = "@sync-c")]
  sync_c: Option<bool>,
}

/// Schema defined at https://gerrit.googlesource.com/git-repo/+/HEAD/docs/manifest-format.md#Element-remote
#[derive(Debug, Deserialize)]
struct ProjectSchema {
  #[serde(rename = "@name")]
  name: String,

  #[serde(rename = "@path")]
  path: Option<String>,

  #[serde(rename = "@remote")]
  remote: Option<String>,

  #[serde(rename = "@revision")]
  revision: Option<String>,

  #[serde(rename = "@dest-branch")]
  dest_branch: Option<String>,

  #[serde(rename = "@groups")]
  groups: Option<Vec<String>>,

  #[serde(rename = "@sync-c")]
  sync_c: Option<bool>,

  #[serde(rename = "@sync-s")]
  sync_s: Option<bool>,

  #[serde(rename = "@upstream")]
  upstream: Option<String>,

  #[serde(rename = "@clone-depth")]
  clone_depth: Option<u32>,

  #[serde(rename = "@force-path")]
  force_path: Option<bool>,

  #[serde(rename = "annotation")]
  annotations: Option<Vec<AnnotationSchema>>,

  #[serde(rename = "copyfile")]
  copy_files: Option<Vec<CopyFileSchema>>,

  #[serde(rename = "linkfile")]
  link_files: Option<Vec<LinkFileSchema>>,
}

/// Schema defined at https://gerrit.googlesource.com/git-repo/+/HEAD/docs/manifest-format.md#element-copyfile
#[derive(Debug, Deserialize)]
struct CopyFileSchema {
  #[serde(rename = "@src")]
  src: String,

  #[serde(rename = "@dest")]
  dest: String,
}

/// Schema defined at https://gerrit.googlesource.com/git-repo/+/HEAD/docs/manifest-format.md#element-annotation
#[derive(Debug, Deserialize)]
struct AnnotationSchema {
  #[serde(rename = "@name")]
  name: String,

  #[serde(rename = "@value")]
  value: String,
}

/// Schema defined at https://gerrit.googlesource.com/git-repo/+/HEAD/docs/manifest-format.md#element-linkfile
#[derive(Debug, Deserialize)]
struct LinkFileSchema {
  #[serde(rename = "@src")]
  src: String,

  #[serde(rename = "@dest")]
  dest: String,
}

/// Schema defined at https://gerrit.googlesource.com/git-repo/+/HEAD/docs/manifest-format.md#Element-include
#[derive(Debug, Deserialize)]
struct IncludeSchema {
  #[serde(rename = "@name")]
  name: String,
}

#[derive(Default, Debug)]
pub struct Manifest {
  pub remotes: HashMap<String, Remote>,
  pub projects: BTreeMap<PathBuf, Project>,
  pub default: Option<Default>,
  pub manifest_server: Option<ManifestServer>,
  pub superproject: Option<SuperProject>,
  pub contactinfo: Option<ContactInfo>,
  pub repo_hooks: Option<RepoHooks>,
}

impl Manifest {
  fn construct_from_schema(schema: ManifestSchema, manifest_root: impl AsRef<Path>) -> anyhow::Result<Self> {
    let ManifestSchema {
      project,
      remote,
      default,
      include,
    } = schema;

    Ok(Self {
      remotes: remote
        .into_iter()
        .map(|remote| (remote.name.clone(), remote.into()))
        .collect(),
      projects: project
        .into_iter()
        .map(|project_schema| {
          let path = PathBuf::from(project_schema.path.as_ref().unwrap_or_else(|| &project_schema.name));
          (path, project_schema.into())
        })
        .collect(),
      default: default.map(|value| value.into()),
      manifest_server: None,
      superproject: None,
      contactinfo: None,
      repo_hooks: None,
    })
  }
}

#[derive(Default, Debug)]
pub struct Remote {
  pub name: String,
  pub alias: Option<String>,
  pub fetch: String,
  pub push_url: Option<String>,
  pub review: Option<String>,
  pub revision: Option<String>,
}

impl From<RemoteSchema> for Remote {
  fn from(remote: RemoteSchema) -> Self {
    Self {
      name: remote.name,
      alias: remote.alias,
      fetch: remote.fetch,
      push_url: remote.push_url,
      review: remote.review,
      revision: remote.revision,
    }
  }
}

#[derive(Default, Debug)]
pub struct Default {
  pub remote: Option<String>,
  pub revision: Option<String>,
  pub dest_branch: Option<String>,
  pub upstream: Option<String>,
  pub sync_j: Option<u32>,
  pub sync_c: Option<bool>,
}

impl From<DefaultSchema> for Default {
  fn from(default: DefaultSchema) -> Self {
    Self {
      remote: default.remote,
      revision: default.revision,
      dest_branch: default.dest_branch,
      upstream: default.upstream,
      sync_j: default.sync_j,
      sync_c: default.sync_c,
    }
  }
}

#[derive(Debug)]
pub struct ManifestServer {
  pub url: String,
}

#[derive(Debug)]
pub struct SuperProject {
  #[allow(dead_code)]
  pub name: String,
  #[allow(dead_code)]
  pub remote: String,
}

#[derive(Debug)]
pub struct ContactInfo {
  #[allow(dead_code)]
  pub bug_url: String,
}

#[derive(Clone, Default, Debug)]
pub struct Project {
  pub name: String,
  pub path: Option<String>,
  pub remote: Option<String>,
  pub revision: Option<String>,

  pub dest_branch: Option<String>,
  pub groups: Option<Vec<String>>,

  pub sync_c: Option<bool>,
  pub clone_depth: Option<u32>,

  pub file_operations: Vec<FileOperation>,
  pub annotations: HashMap<String, String>,
}

impl From<ProjectSchema> for Project {
  fn from(schema: ProjectSchema) -> Self {
    Self {
      name: schema.name,
      path: schema.path,
      remote: schema.remote,
      revision: schema.revision,
      dest_branch: schema.dest_branch,
      groups: schema.groups,
      sync_c: schema.sync_c,
      clone_depth: schema.clone_depth,
      file_operations: schema
        .copy_files
        .unwrap_or_default()
        .into_iter()
        .map(FileOperation::from)
        .chain(
          schema
            .link_files
            .unwrap_or_default()
            .into_iter()
            .map(FileOperation::from),
        )
        .collect(),
      annotations: schema
        .annotations
        .unwrap_or_default()
        .into_iter()
        .map(|annotation| (annotation.name, annotation.value))
        .collect(),
    }
  }
}

impl Project {
  pub fn path(&self) -> &str {
    self.path.as_ref().unwrap_or(&self.name)
  }

  pub fn find_remote(&self, manifest: &Manifest) -> Result<String, Error> {
    let remote_name = self
      .remote
      .as_ref()
      .or_else(|| manifest.default.as_ref().and_then(|default| default.remote.as_ref()))
      .ok_or_else(|| format_err!("project {} has no remote", self.name))?
      .clone();

    Ok(remote_name)
  }

  pub fn find_revision(&self, manifest: &Manifest) -> Result<String, Error> {
    if let Some(revision) = &self.revision {
      return Ok(revision.clone());
    }

    if let Some(default) = &manifest.default {
      if let Some(revision) = &default.revision {
        return Ok(revision.clone());
      }
    }

    let remote_name = self.find_remote(manifest)?;
    manifest
      .remotes
      .get(&remote_name)
      .as_ref()
      .and_then(|remote| remote.revision.as_ref())
      .cloned()
      .ok_or_else(|| format_err!("project {} has no revision", self.name))
  }

  pub fn find_dest_branch(&self, manifest: &Manifest) -> Result<String, Error> {
    // repo seems to only look at project to calculate dest_branch, but that seems wrong.
    let dest_branch = self
      .dest_branch
      .clone()
      .ok_or(())
      .or_else(|_| self.find_revision(manifest))
      .with_context(|| format!("project {} has no dest_branch or revision", self.name))?;

    Ok(dest_branch)
  }
}

#[derive(Default, Debug)]
pub struct ExtendProject {
  pub name: String,
  pub path: Option<String>,
  pub groups: Option<Vec<String>>,
  pub revision: Option<String>,
  pub remote: Option<String>,
}

impl ExtendProject {
  pub fn extend(&self, project: &Project) -> Project {
    // Limit changes to projects at the specified path
    if let Some(path) = &self.path {
      if *path != project.path() {
        return project.clone();
      }
    }

    let mut extended = project.clone();

    if let Some(groups) = &self.groups {
      let mut old_groups = project.groups.clone().unwrap_or_default();
      old_groups.extend(groups.clone());
      extended.groups = Some(old_groups);
    }

    if let Some(revision) = &self.revision {
      extended.revision = Some(revision.clone());
    }

    if let Some(remote) = &self.remote {
      extended.remote = Some(remote.clone());
    }

    extended
  }
}

#[derive(Clone, Debug)]
pub enum FileOperation {
  LinkFile { src: String, dst: String },
  CopyFile { src: String, dst: String },
}

impl FileOperation {
  pub fn src(&self) -> &str {
    match self {
      FileOperation::LinkFile { src, .. } => src,
      FileOperation::CopyFile { src, .. } => src,
    }
  }

  pub fn dst(&self) -> &str {
    match self {
      FileOperation::LinkFile { dst, .. } => dst,
      FileOperation::CopyFile { dst, .. } => dst,
    }
  }
}

impl From<CopyFileSchema> for FileOperation {
  fn from(schema: CopyFileSchema) -> Self {
    Self::CopyFile {
      src: schema.src,
      dst: schema.dest,
    }
  }
}

impl From<LinkFileSchema> for FileOperation {
  fn from(schema: LinkFileSchema) -> Self {
    Self::LinkFile {
      src: schema.src,
      dst: schema.dest,
    }
  }
}

#[derive(Default, Debug)]
pub struct RepoHooks {
  pub in_project: Option<String>,
  pub enabled_list: Option<String>,
}

fn canonicalize_url(url: &str) -> &str {
  url.trim_end_matches('/')
}

impl Manifest {
  pub fn parse(manifest_root: impl AsRef<Path>, default_manifest: impl AsRef<Path>) -> Result<Manifest, Error> {
    let default_manifest_file = File::open(default_manifest)?;
    let manifest_schema: ManifestSchema = quick_xml::de::from_reader(BufReader::new(default_manifest_file))?;
    Ok(Manifest::construct_from_schema(manifest_schema, manifest_root)?)
  }

  pub fn serialize(&self, output: Box<dyn Write>) -> Result<(), Error> {
    unimplemented!()
  }

  pub fn resolve_project_remote(
    &self,
    config: &Config,
    tree_config: &TreeConfig,
    project: &Project,
  ) -> Result<(String, &Remote), Error> {
    let project_remote_name = project.find_remote(self)?;
    let project_remote = self
      .remotes
      .get(&project_remote_name)
      .ok_or_else(|| format_err!("remote {} missing in manifest", project_remote_name))?;

    // repo allows the use of ".." to mean the URL from which the manifest was cloned.
    if project_remote.fetch == ".." {
      return Ok((tree_config.remote.clone(), project_remote));
    }

    let url = canonicalize_url(&project_remote.fetch);
    for remote in &config.remotes {
      if url == canonicalize_url(&remote.url) {
        return Ok((remote.name.clone(), project_remote));
      }
      for other_url in remote.other_urls.as_deref().unwrap_or(&[]) {
        if url == canonicalize_url(other_url) {
          return Ok((remote.name.clone(), project_remote));
        }
      }
    }

    Err(format_err!("couldn't find remote in configuration matching '{}'", url))
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_basic_schema() -> anyhow::Result<()> {
    const MANIFEST: &str = r#"
<?xml version="1.0" encoding="UTF-8"?>
<manifest>
  <remote name="aosp" fetch="ssh://git-repos.com/data/gitrepos" />
  <project name="vendor/hello-world" path="/root/vendor/hello-world" />
</manifest>
"#;

    let manifest: ManifestSchema = quick_xml::de::from_str(MANIFEST)?;

    assert_eq!(manifest.remote.len(), 1);
    assert_eq!(manifest.remote[0].name, "aosp".to_owned());
    assert_eq!(manifest.remote[0].fetch, "ssh://git-repos.com/data/gitrepos".to_owned());
    assert_eq!(manifest.remote[0].push_url, None);
    assert_eq!(manifest.remote[0].review, None);
    assert_eq!(manifest.remote[0].revision, None);

    assert_eq!(manifest.project.len(), 1);
    assert_eq!(manifest.project[0].name, "vendor/hello-world".to_owned());
    assert_eq!(manifest.project[0].path, Some("/root/vendor/hello-world".to_owned()));
    assert_eq!(manifest.project[0].remote, None);
    assert_eq!(manifest.project[0].revision, None);
    assert_eq!(manifest.project[0].dest_branch, None);
    assert_eq!(manifest.project[0].groups, None);
    assert_eq!(manifest.project[0].sync_c, None);
    assert_eq!(manifest.project[0].clone_depth, None);

    assert!(manifest.default.is_none());

    Ok(())
  }

  #[test]
  fn test_basic_schema_with_default() -> anyhow::Result<()> {
    const MANIFEST: &str = r#"
<?xml version="1.0" encoding="UTF-8"?>
<manifest>
  <remote name="aosp" fetch="ssh://git-repos.com/data/gitrepos" />
  <default revision="master" remote="aosp" />
  <project name="vendor/hello-world" path="/root/vendor/hello-world" >
    <annotation name="prebuilt_manifest" value="prebuilt_manifest.json"/>
    <annotation name="prebuilt_manifest_type" value="apps"/>
  </project>
  <project name="aosp/platform/build" path="build/make">
    <linkfile src="CleanSpec.mk" dest="build/CleanSpec.mk"/>
    <copyfile src="core/root.mk" dest="Makefile"/>
    <linkfile src="envsetup.sh" dest="build/envsetup.sh"/>
  </project>
</manifest>
"#;

    let manifest: ManifestSchema = quick_xml::de::from_str(MANIFEST)?;

    assert_eq!(manifest.remote.len(), 1);
    assert_eq!(manifest.remote[0].name, "aosp".to_owned());
    assert_eq!(manifest.remote[0].fetch, "ssh://git-repos.com/data/gitrepos".to_owned());
    assert_eq!(manifest.remote[0].push_url, None);
    assert_eq!(manifest.remote[0].review, None);
    assert_eq!(manifest.remote[0].revision, None);

    assert_eq!(manifest.project.len(), 2);
    assert_eq!(manifest.project[0].name, "vendor/hello-world".to_owned());
    assert_eq!(manifest.project[0].path, Some("/root/vendor/hello-world".to_owned()));
    assert_eq!(manifest.project[0].remote, None);
    assert_eq!(manifest.project[0].revision, None);
    assert_eq!(manifest.project[0].dest_branch, None);
    assert_eq!(manifest.project[0].groups, None);
    assert_eq!(manifest.project[0].sync_c, None);
    assert_eq!(manifest.project[0].clone_depth, None);

    let default = manifest.default.as_ref().expect("Default tag is missing");
    assert_eq!(default.remote, Some("aosp".to_owned()));
    assert_eq!(default.revision, Some("master".to_owned()));

    Ok(())
  }

  #[test]
  fn test_project() -> anyhow::Result<()> {
    const MANIFEST: &str = r#"
<?xml version="1.0" encoding="UTF-8"?>
<manifest>
  <remote name="aosp" fetch="ssh://git-repos.com/data/gitrepos" />
  <remote name="special-remote" fetch="ssh://git-repos.com/data/gitrepos" />
  <default revision="master" remote="aosp" />
  <project name="vendor/hello-world" path="/root/vendor/hello-world" />
  <project name="vendor/foo-bar" path="/root/vendor/foo-bar" revision="special-revision" />
  <project name="vendor/dead-beef" path="/root/vendor/dead-beef" remote="special-remote" />
</manifest>
"#;

    let manifest_schema: ManifestSchema = quick_xml::de::from_str(MANIFEST)?;
    let manifest = Manifest::construct_from_schema(manifest_schema, "")?;

    assert_eq!(manifest.projects.len(), 3);

    let hello_world = manifest
      .projects
      .get(&PathBuf::from("/root/vendor/hello-world"))
      .expect("Missing project 'hello-world'");
    assert_eq!(hello_world.name, "vendor/hello-world");
    assert_eq!(hello_world.path, Some("/root/vendor/hello-world".to_owned()));
    assert_eq!(hello_world.find_remote(&manifest)?, "aosp".to_owned());
    assert_eq!(hello_world.find_revision(&manifest)?, "master".to_owned());

    let foo_bar = manifest
      .projects
      .get(&PathBuf::from("/root/vendor/foo-bar"))
      .expect("Missing project 'foo-bar'");
    assert_eq!(foo_bar.name, "vendor/foo-bar");
    assert_eq!(foo_bar.path, Some("/root/vendor/foo-bar".to_owned()));
    assert_eq!(foo_bar.find_remote(&manifest)?, "aosp".to_owned());
    assert_eq!(foo_bar.find_revision(&manifest)?, "special-revision".to_owned());

    let foo_bar = manifest
      .projects
      .get(&PathBuf::from("/root/vendor/dead-beef"))
      .expect("Missing project 'dead-beef'");
    assert_eq!(foo_bar.name, "vendor/dead-beef");
    assert_eq!(foo_bar.path, Some("/root/vendor/dead-beef".to_owned()));
    assert_eq!(foo_bar.find_remote(&manifest)?, "special-remote".to_owned());
    assert_eq!(foo_bar.find_revision(&manifest)?, "master".to_owned());

    Ok(())
  }
}
