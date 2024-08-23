# Contributing to the ALPHA-g data handler

## Pull Requests

All contributions to the ALPHA-g data handler happen through pull requests. It
is highly recommended to discuss any changes by opening an
[issue](https://github.com/ALPHA-g-Experiment/data-handler/issues) before you
start working on a pull request. Additionally, please look at the GitHub actions
[workflows](https://github.com/ALPHA-g-Experiment/data-handler/tree/main/.github/workflows)
to find out all the checks that your code has to pass before it can be
reviewed/merged.

## Release Process

Once you have implemented all the fixes/features you want to release (make sure
you are on the main branch and it is up-to-date), you need to:

```bash
# Step 1: Make a new branch
git switch -c $NEW_BRANCH

# Step 2: Check all commits since the last release
cargo release changes

# Step 3: Update the `CHANGELOG.md` to include anything relevant that was missed
# Then commit the changes e.g.
git commit -am "docs: Update CHANGELOG.md"

# Step 4: Determine what the next version is (according to semver) and give
# `cargo-release` the pleasure of doing all the boring stuff
cargo release --execute --no-publish --no-tag --allow-branch=$NEW_BRANCH $NEW_VERSION

# Step 5: Open a pull request and review/merge to `main`

# Step 6: Release from the main branch
git checkout main
git pull
cargo release tag --execute
cargo release push --execute
```

After following these steps, please go to the GitHub releases page and manually
rename the new release to match the tag name (see
[this issue](https://github.com/axodotdev/cargo-dist/issues/1286)).
