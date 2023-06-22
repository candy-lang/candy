# Packages and Workspaces

## Only packages

```text
_package.candy
_.candy
common/
  ...
app/
  _build.candy
  _.candy
  ...
dashboard/
  ...
third_party_package/
  _package.candy
  ...
```

Packages are self-contained and can possibly be built and published.

## Packages and workspaces

```text
_workspace.candy
common/
  _package.candy
app/
  ...
dashboard/
  ...
third_party_package/
  _workspace.candy
  _package.candy
```

Packages can be built and published.
Workspaces are self-contained.
