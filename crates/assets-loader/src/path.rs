pub use std::path::Path;
use std::{
    borrow::{Borrow, Cow},
    iter::FusedIterator,
    ops::Deref,
    str::FromStr,
};

#[repr(transparent)]
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AssetPathBuf(String);

#[repr(transparent)]
#[derive(PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AssetPath(str);

impl AssetPathBuf {
    #[inline]
    pub const fn new() -> Self {
        Self(String::new())
    }

    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        Self(String::with_capacity(capacity))
    }

    #[inline]
    pub fn as_path(&self) -> &AssetPath {
        self
    }

    #[inline]
    pub fn push<P: AsRef<AssetPath>>(&mut self, path: P) {
        self._push(path.as_ref())
    }

    fn _push(&mut self, path: &AssetPath) {
        if path.0.is_empty() {
            return;
        }
        // absolute `path` replaces `self`
        if path.is_absolute() {
            self.0.truncate(0);
        } else {
            // trim fragment
            if let Some(pos) = self.0.find('#') {
                self.0.truncate(pos);
            }

            // push missing seperator
            if !self.0.is_empty() && !self.0.ends_with(AssetPath::is_seperator) {
                self.0.push('/');
            }
        }
        self.0.reserve(path.0.len());
        for comp in path.components() {
            match comp {
                Component::RootDir => {
                    self.0.truncate(0);
                    self.0.push('/');
                }
                Component::ParentDir => {
                    if !self.pop() && self.0.is_empty() {
                        self.0.push_str("../");
                    }
                }
                Component::File(f) => self.0.push_str(f),
                Component::Directory(f) => {
                    self.0.push_str(f);
                    self.0.push('/');
                }
                Component::Fragment(f) => {
                    self.0.push('#');
                    self.0.push_str(f);
                }
            }
        }
    }

    pub fn pop(&mut self) -> bool {
        let Some(parent) = self.parent() else {
            return false;
        };
        let len = parent.0.trim_end_matches(AssetPath::is_seperator).len();
        self.0.truncate(len + 1);
        true
    }

    #[inline]
    pub fn set_file_name<S: AsRef<str>>(&mut self, file_name: S) {
        self._set_file_name(file_name.as_ref())
    }

    fn _set_file_name(&mut self, file_name: &str) {
        if self.file_name().is_some() {
            let popped = self.pop();
            debug_assert!(popped);
        }
        self.push(file_name);
    }

    #[inline]
    pub fn set_fragment<S: AsRef<str>>(&mut self, fragment: S) {
        self._set_fragment(fragment.as_ref())
    }

    fn _set_fragment(&mut self, fragment: &str) {
        if let Some(pos) = self.0.find('#') {
            self.0.truncate(pos + 1);
        } else {
            self.0.push('#');
        }
        self.0.push_str(fragment);
    }

    #[inline]
    pub fn into_string(self) -> String {
        self.0
    }

    #[inline]
    pub fn capacity(&self) -> usize {
        self.0.capacity()
    }

    #[inline]
    pub fn clear(&mut self) {
        self.0.clear()
    }

    #[inline]
    pub fn reserve(&mut self, additional: usize) {
        self.0.reserve(additional)
    }

    #[inline]
    pub fn reserve_exact(&mut self, additional: usize) {
        self.0.reserve_exact(additional)
    }

    #[inline]
    pub fn shrink_to_fit(&mut self) {
        self.0.shrink_to_fit()
    }
}

impl Deref for AssetPathBuf {
    type Target = AssetPath;
    #[inline]
    fn deref(&self) -> &Self::Target {
        AssetPath::new(&self.0)
    }
}

impl<T: ?Sized + AsRef<str>> From<&T> for AssetPathBuf {
    #[inline]
    fn from(s: &T) -> Self {
        Self::from(s.as_ref().to_string())
    }
}

impl From<String> for AssetPathBuf {
    #[inline]
    fn from(s: String) -> Self {
        Self(s)
    }
}
impl From<AssetPathBuf> for String {
    #[inline]
    fn from(path_buf: AssetPathBuf) -> Self {
        path_buf.0
    }
}
impl FromStr for AssetPathBuf {
    type Err = core::convert::Infallible;

    #[inline]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self::from(s))
    }
}

impl Borrow<AssetPath> for AssetPathBuf {
    #[inline]
    fn borrow(&self) -> &AssetPath {
        self.deref()
    }
}

impl Default for AssetPathBuf {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl AssetPath {
    #[inline]
    fn is_seperator(c: char) -> bool {
        c == '/' || c == '\\'
    }

    #[inline]
    pub fn new<S: AsRef<str> + ?Sized>(s: &S) -> &Self {
        let s: *const str = s.as_ref();
        unsafe { &*(s as *const Self) }
    }

    #[inline]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn to_path_buf(&self) -> AssetPathBuf {
        AssetPathBuf::from(self.0.to_string())
    }

    #[inline]
    pub fn is_absolute(&self) -> bool {
        self.0.starts_with(Self::is_seperator)
    }

    #[inline]
    pub fn is_relative(&self) -> bool {
        !self.is_absolute()
    }

    #[inline]
    pub fn is_directory(&self) -> bool {
        let path = if let Some(pos) = self.0.find('#') {
            &self.0[..pos]
        } else {
            &self.0
        };
        path.ends_with(Self::is_seperator)
    }

    #[inline]
    fn split_fragment(&self) -> (&str, Option<&str>) {
        if let Some(pos) = self.0.find('#') {
            (&self.0[..pos], Some(&self.0[pos + 1..]))
        } else {
            (&self.0, None)
        }
    }

    #[inline]
    fn split_parent_fragment(&self) -> (Option<&str>, &str, Option<&str>) {
        let (path, frag) = self.split_fragment();
        if let Some(pos) = path
            .trim_end_matches(Self::is_seperator)
            .rfind(Self::is_seperator)
        {
            let parent = &path[..pos + 1];
            (Some(parent), &path[parent.len()..], frag)
        } else {
            (None, path, frag)
        }
    }

    pub fn parent(&self) -> Option<&Self> {
        let (parent, _file, _frag) = self.split_parent_fragment();
        parent.map(Self::new)
    }

    pub fn file_name(&self) -> Option<&str> {
        let (_parent, file, _frag) = self.split_parent_fragment();
        let file = file.trim_end_matches(Self::is_seperator);
        if file.is_empty() {
            None
        } else {
            Some(file)
        }
    }

    #[inline]
    fn split_parent_file_extension_fragment(
        &self,
    ) -> (Option<&str>, Option<&str>, Option<&str>, Option<&str>) {
        let (parent, file, frag) = self.split_parent_fragment();
        if file.ends_with(Self::is_seperator) {
            let filename = file.trim_end_matches(Self::is_seperator);
            if filename.is_empty() {
                (parent, None, None, frag)
            } else {
                (parent, Some(filename), None, frag)
            }
        } else if file.starts_with('.') {
            (parent, Some(file), None, frag)
        } else if let Some(pos) = file.find('.') {
            (parent, Some(&file[..pos]), Some(&file[pos..]), frag)
        } else {
            (parent, Some(file), None, frag)
        }
    }

    #[inline]
    pub fn file_stem(&self) -> Option<&str> {
        let (_, stem, _, _) = self.split_parent_file_extension_fragment();
        stem
    }

    #[inline]
    pub fn extension(&self) -> Option<&str> {
        let (_, _, ext, _) = self.split_parent_file_extension_fragment();
        ext
    }

    #[inline]
    pub fn fragment(&self) -> Option<&str> {
        let (_path, frag) = self.split_fragment();
        frag
    }

    #[inline]
    pub fn components(&self) -> Components<'_> {
        Components {
            path: &self.0,
            state: State::Start,
        }
    }

    pub fn normalize(&self) -> AssetPathBuf {
        let mut res = AssetPathBuf::new();
        res.push(self);
        res
    }

    pub fn iter(&self) -> Iter<'_> {
        Iter(self.components())
    }

    pub fn join<P: AsRef<Self>>(&self, path: P) -> AssetPathBuf {
        self._join(path.as_ref())
    }

    fn _join(&self, path: &Self) -> AssetPathBuf {
        let mut buf = if path.is_absolute() {
            AssetPathBuf::new()
        } else {
            self.to_path_buf()
        };
        buf.push(path);
        buf
    }

    pub fn with_file_name<S: AsRef<str>>(&self, file_name: S) -> AssetPathBuf {
        self._with_file_name(file_name.as_ref())
    }

    fn _with_file_name(&self, file_name: &str) -> AssetPathBuf {
        let mut buf = self.to_path_buf();
        buf.set_file_name(file_name);
        buf
    }

    pub fn with_fragment<S: AsRef<str>>(&self, fragment: S) -> AssetPathBuf {
        self._with_fragment(fragment.as_ref())
    }

    fn _with_fragment(&self, fragment: &str) -> AssetPathBuf {
        let mut buf = self.to_path_buf();
        buf.set_fragment(fragment);
        buf
    }
}

impl std::fmt::Debug for AssetPath {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(&self.0, formatter)
    }
}

impl std::fmt::Display for AssetPath {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.0, formatter)
    }
}

impl ToOwned for AssetPath {
    type Owned = AssetPathBuf;
    #[inline]
    fn to_owned(&self) -> Self::Owned {
        self.to_path_buf()
    }
}

impl AsRef<str> for AssetPath {
    #[inline]
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl AsRef<Path> for AssetPath {
    #[inline]
    fn as_ref(&self) -> &Path {
        self.0.as_ref()
    }
}

impl AsRef<Self> for AssetPath {
    #[inline]
    fn as_ref(&self) -> &Self {
        self
    }
}

impl AsRef<AssetPath> for AssetPathBuf {
    #[inline]
    fn as_ref(&self) -> &AssetPath {
        self
    }
}

impl AsRef<str> for AssetPathBuf {
    #[inline]
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl AsRef<Path> for AssetPathBuf {
    #[inline]
    fn as_ref(&self) -> &Path {
        self.0.as_ref()
    }
}

impl AsRef<AssetPath> for str {
    #[inline]
    fn as_ref(&self) -> &AssetPath {
        AssetPath::new(self)
    }
}

impl AsRef<AssetPath> for Cow<'_, str> {
    #[inline]
    fn as_ref(&self) -> &AssetPath {
        AssetPath::new(self)
    }
}
impl AsRef<AssetPath> for String {
    #[inline]
    fn as_ref(&self) -> &AssetPath {
        AssetPath::new(self)
    }
}

impl<'a> IntoIterator for &'a AssetPathBuf {
    type Item = &'a str;
    type IntoIter = Iter<'a>;
    #[inline]
    fn into_iter(self) -> Iter<'a> {
        self.iter()
    }
}

impl<'a> IntoIterator for &'a AssetPath {
    type Item = &'a str;
    type IntoIter = Iter<'a>;
    #[inline]
    fn into_iter(self) -> Iter<'a> {
        self.iter()
    }
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Component<'a> {
    RootDir,
    ParentDir,
    Directory(&'a str),
    File(&'a str),
    Fragment(&'a str),
}

impl<'a> Component<'a> {
    pub fn as_str(self) -> &'a str {
        match self {
            Component::RootDir => "/",
            Component::ParentDir => "..",
            Component::Directory(s) | Component::File(s) | Component::Fragment(s) => s,
        }
    }

    fn from_str(comp: &'a str, is_dir: bool) -> Option<Self> {
        if comp.starts_with('#') {
            Some(Component::Fragment(&comp[1..]))
        } else if comp == ".." {
            Some(Component::ParentDir)
        } else if !comp.is_empty() && comp != "." {
            if is_dir {
                Some(Component::Directory(comp))
            } else {
                Some(Component::File(comp))
            }
        } else {
            None
        }
    }
}

impl AsRef<str> for Component<'_> {
    #[inline]
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl AsRef<AssetPath> for Component<'_> {
    #[inline]
    fn as_ref(&self) -> &AssetPath {
        self.as_str().as_ref()
    }
}

#[derive(Clone)]
pub struct Components<'a> {
    // The path left to parse components from
    path: &'a str,

    state: State,
}

#[derive(Copy, Clone, PartialEq, PartialOrd, Debug)]
enum State {
    Start = 1, // / or . or nothing
    Body = 2,  // foo/bar/baz
    Done = 4,
}

impl<'a> Components<'a> {
    pub fn as_path(&self) -> &'a AssetPath {
        AssetPath::new(self.path)
    }
}

impl AsRef<AssetPath> for Components<'_> {
    #[inline]
    fn as_ref(&self) -> &AssetPath {
        self.as_path()
    }
}

impl AsRef<str> for Components<'_> {
    #[inline]
    fn as_ref(&self) -> &str {
        self.as_path().as_str()
    }
}

impl<'a> Iterator for Components<'a> {
    type Item = Component<'a>;

    fn next(&mut self) -> Option<Component<'a>> {
        loop {
            match self.state {
                State::Done => return None,
                State::Start => {
                    self.state = State::Body;
                    if self.path.starts_with(AssetPath::is_seperator) {
                        self.path = self.path.trim_start_matches(AssetPath::is_seperator);
                        return Some(Component::RootDir);
                    }
                }
                State::Body => {
                    if self.path.is_empty() {
                        self.state = State::Done;
                        return None;
                    }
                    if self.path.starts_with('#') {
                        self.state = State::Done;
                        return Some(Component::Fragment(&self.path[1..]));
                    }
                    let mut is_dir = false;
                    let comp;
                    if let Some(pos) = self.path.find(|c| AssetPath::is_seperator(c) || c == '#') {
                        comp = &self.path[..pos];
                        self.path = &self.path[pos..];
                        if self.path.starts_with(AssetPath::is_seperator) {
                            is_dir = true;
                            self.path = self.path.trim_start_matches(AssetPath::is_seperator);
                        }
                    } else {
                        // last component
                        self.state = State::Done;
                        comp = self.path;
                    }
                    if let Some(comp) = Component::from_str(comp, is_dir) {
                        return Some(comp);
                    }
                }
            }
        }
    }
}

impl<'a> DoubleEndedIterator for Components<'a> {
    fn next_back(&mut self) -> Option<Component<'a>> {
        loop {
            match self.state {
                State::Done => return None,
                State::Start => {
                    self.state = State::Body;
                    if let Some(pos) = self.path.find('#') {
                        let frag = &self.path[pos + 1..];
                        self.path = &self.path[..pos];
                        return Some(Component::Fragment(frag));
                    }
                }
                State::Body => {
                    let mut is_dir = false;
                    if self.path.ends_with(AssetPath::is_seperator) {
                        self.path = self.path.trim_end_matches(AssetPath::is_seperator);
                        is_dir = true;
                    }
                    if self.path.is_empty() {
                        self.state = State::Done;
                        if is_dir {
                            return Some(Component::RootDir);
                        } else {
                            return None;
                        }
                    }

                    let comp;
                    if let Some(pos) = self.path.rfind(AssetPath::is_seperator) {
                        comp = &self.path[pos + 1..];
                        self.path = &self.path[..pos + 1];
                    } else {
                        // last component
                        comp = self.path;
                        self.state = State::Done;
                    }
                    if let Some(comp) = Component::from_str(comp, is_dir) {
                        return Some(comp);
                    }
                }
            }
        }
    }
}

impl FusedIterator for Components<'_> {}

#[derive(Clone)]
pub struct Iter<'a>(Components<'a>);

impl<'a> Iter<'a> {
    #[inline]
    pub fn as_path(&self) -> &'a AssetPath {
        self.0.as_path()
    }
}

impl AsRef<AssetPath> for Iter<'_> {
    #[inline]
    fn as_ref(&self) -> &AssetPath {
        self.as_path()
    }
}

impl AsRef<str> for Iter<'_> {
    #[inline]
    fn as_ref(&self) -> &str {
        self.as_path().as_str()
    }
}

impl<'a> Iterator for Iter<'a> {
    type Item = &'a str;
    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(Component::as_str)
    }
}

impl<'a> DoubleEndedIterator for Iter<'a> {
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        self.0.next_back().map(Component::as_str)
    }
}

impl FusedIterator for Iter<'_> {}
