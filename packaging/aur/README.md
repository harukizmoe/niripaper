# AUR

`PKGBUILD` for [niripaper](https://github.com/harukizmoe/niripaper). It builds from the
tagged source tarball, so it needs a release to exist first.

## Publishing

The AUR is its own git repository, and only its maintainer can push to it — these are the
steps, to be run by hand:

```bash
# 1. Tag and release (the release workflow builds the binary and creates the Release).
git tag v0.2.0 && git push origin v0.2.0

# 2. Fill in the real checksum: GitHub's tag archives are not guaranteed to stay
#    byte-identical, so the PKGBUILD ships with SKIP.
curl -L -o /tmp/niripaper-0.2.0.tar.gz \
  https://github.com/harukizmoe/niripaper/archive/refs/tags/v0.2.0.tar.gz
sha256sum /tmp/niripaper-0.2.0.tar.gz   # put this into PKGBUILD

# 3. Check it builds in a clean chroot, then push.
makepkg --syncdeps --cleanbuild
git clone ssh://aur@aur.archlinux.org/niripaper.git /tmp/aur-niripaper
cp PKGBUILD /tmp/aur-niripaper/
cd /tmp/aur-niripaper
makepkg --printsrcinfo > .SRCINFO      # the AUR needs this alongside PKGBUILD
git add PKGBUILD .SRCINFO && git commit -m "0.2.0" && git push
```

## Dependencies

`depends` is what the binary links by hand, checked with `ldd` and `pacman -Qo`:

| soname | package |
| --- | --- |
| `libmpv.so.2` | `mpv` |
| `libEGL.so.1`, `libGL.so.1` | `libglvnd` |
| `libgbm.so.1` | `mesa` |
| `libwayland-client.so.0` | `wayland` |
