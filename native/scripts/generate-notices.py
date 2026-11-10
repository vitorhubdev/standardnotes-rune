#!/usr/bin/env python3
"""Collect exact license texts for the locked native Linux/Windows dependency graphs.

Requires Python 3.11+ and Cargo. This script never compiles or installs tooling.
By default Cargo operates offline; --online only permits downloading crate sources.
Upstream licenses omitted from crate archives are checked-in, revision-pinned inputs.
"""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess
import tomllib
from urllib.parse import urlsplit


NATIVE = Path(__file__).resolve().parents[1]
OUTPUT = NATIVE / "third-party"
TARGETS = ("x86_64-unknown-linux-gnu", "x86_64-pc-windows-msvc")
ROOT_PACKAGE = "standardnotes-rune"
LICENSE_NAME = re.compile(r"(?:^|[._-])(?:licen[cs]e|copying|copyright|notice|unlicense|authors)(?:$|[._-])", re.I)
RUNE_ASSETS = {
    "Rune-MIT.txt": "LICENSE",
    "Inter-OFL.txt": "crates/rune/assets/Inter-LICENSE.txt",
    "IBMPlexMono-OFL.txt": "crates/rune/assets/IBMPlexMono-LICENSE.txt",
    "Rune-highlights-Apache.txt": "crates/rune-highlights/vendor/LICENSE.md",
}


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def repository_text_sha256(path: Path) -> str:
    # Git may use CRLF in a Windows worktree. Sources are recorded with LF.
    return sha256(path.read_bytes().replace(b"\r\n", b"\n"))


def write_json(path: Path, value: object) -> None:
    path.write_text(json.dumps(value, indent=2, ensure_ascii=False, sort_keys=True) + "\n", encoding="utf-8", newline="\n")


def package_key(package: dict) -> str:
    return f"{package['name']}@{package['version']}|{package['source'] or 'workspace'}"


def github_repository(url: str | None) -> str | None:
    if not url:
        return None
    parsed = urlsplit(url.removeprefix("git+"))
    parts = parsed.path.strip("/").split("/")
    if parsed.hostname == "github.com" and len(parts) >= 2:
        return f"https://github.com/{parts[0]}/{parts[1].removesuffix('.git')}"
    return None


def git_root(base: Path) -> Path:
    for directory in (base, *base.parents):
        if (directory / ".git").exists():
            return directory
    raise ValueError(f"Git checkout root missing for {base}")


def license_files(base: Path) -> list[Path]:
    files = []
    for path in base.rglob("*"):
        if not path.is_file() or not LICENSE_NAME.search(path.name):
            continue
        if path.suffix.lower() in (".rs", ".c", ".h", ".py", ".js", ".html", ".svg", ".png"):
            continue
        if any(part in (".git", "target", "node_modules") for part in path.relative_to(base).parts):
            continue
        files.append(path)
    return sorted(files)


def read_metadata(target: str, supplied: dict[str, Path], online: bool) -> dict:
    if target in supplied:
        return json.loads(supplied[target].read_text(encoding="utf-8"))
    command = ["cargo", "metadata", "--locked", "--format-version", "1", "--filter-platform", target]
    if not online:
        command.append("--offline")
    return json.loads(subprocess.check_output(command, cwd=NATIVE))


def dependency_graph(metadata: dict) -> tuple[dict, dict]:
    packages = {p["id"]: p for p in metadata["packages"]}
    nodes = {n["id"]: n for n in metadata["resolve"]["nodes"]}
    roots = [p["id"] for p in packages.values() if p["name"] == ROOT_PACKAGE and p["source"] is None]
    if len(roots) != 1:
        raise ValueError("Expected one local desktop package")
    roles: dict[str, set[str]] = {}
    pending = [(roots[0], "normal")]
    while pending:
        package_id, role = pending.pop()
        if any("proc-macro" in target["kind"] for target in packages[package_id]["targets"]):
            role = "host-build"
        if role in roles.setdefault(package_id, set()):
            continue
        roles[package_id].add(role)
        for dependency in nodes[package_id]["deps"]:
            for kind in dependency["dep_kinds"]:
                if kind["kind"] == "dev":
                    continue
                child_role = "host-build" if kind["kind"] == "build" or role == "host-build" else "normal"
                pending.append((dependency["pkg"], child_role))
    return {key: packages[key] for key in roles}, roles


def archive_checksum(package: dict, expected: str | None) -> dict:
    base = Path(package["manifest_path"]).parent
    # A registry source directory is registry/src/<registry-id>/<crate-version>.
    archive = base.parent.parent.parent / "cache" / base.parent.name / (base.name + ".crate")
    result = {"cargo_lock_sha256": expected}
    if archive.is_file():
        actual = sha256(archive.read_bytes())
        if expected != actual:
            raise ValueError(f"Crate archive checksum mismatch: {package['name']}")
        result["archive_verified_against_lock"] = True
    else:
        result["archive_verified_against_lock"] = False
    return result


def verify() -> None:
    for line in (OUTPUT / "SHA256SUMS").read_text(encoding="utf-8").splitlines():
        expected, relative = line.split("  ", 1)
        path = (OUTPUT / relative).resolve()
        if not path.is_relative_to(OUTPUT.resolve()) or sha256(path.read_bytes()) != expected:
            raise ValueError(f"Notice artifact checksum mismatch: {relative}")
    manifest = json.loads((OUTPUT / "SOURCE_MANIFEST.json").read_text(encoding="utf-8"))
    for name in ("Cargo.lock", "Cargo.toml", "rust-toolchain.toml"):
        expected = manifest["workspace"].get(name + "_sha256")
        if expected and repository_text_sha256(NATIVE / name) != expected:
            raise ValueError(f"{name} changed; regenerate notices")
    if repository_text_sha256(Path(__file__)) != manifest["generator_sha256"]:
        raise ValueError("Generator changed; regenerate notices")
    for package in manifest["packages"]:
        relative = package.get("workspace_manifest")
        if relative and repository_text_sha256(NATIVE / relative) != package["manifest_sha256"]:
            raise ValueError(f"{relative} changed; regenerate notices")
    container = (OUTPUT / manifest["license_text_container"]).read_bytes()
    if sha256(container) != manifest["license_text_container_sha256"]:
        raise ValueError("License text container changed")
    for expected, location in manifest["license_text_locations"].items():
        offset, length = location["byte_offset"], location["byte_length"]
        if sha256(container[offset:offset + length]) != expected:
            raise ValueError(f"Original license text bytes changed: {expected}")
    print(f"Verified {manifest['package_count_union']} package records and {manifest['license_text_count']} complete original license texts")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--online", action="store_true", help="Permit cargo metadata to download locked crate sources")
    parser.add_argument("--metadata", action="append", default=[], metavar="TARGET=PATH", help="Reuse metadata produced with --filter-platform TARGET")
    parser.add_argument("--verify", action="store_true", help="Verify checked-in notices, original text bytes, and workspace inputs without Cargo or network access")
    args = parser.parse_args()
    if args.verify:
        verify()
        return
    supplied = {target: Path(path) for target, path in (value.split("=", 1) for value in args.metadata)}
    if set(supplied) - set(TARGETS):
        raise ValueError("Unrecognized metadata target")

    lock_data = (NATIVE / "Cargo.lock").read_bytes()
    lock = tomllib.loads(lock_data.decode())
    checksums = {(p["name"], p["version"], p.get("source")): p.get("checksum") for p in lock["package"]}
    upstream = json.loads((OUTPUT / "UPSTREAM_LICENSES.json").read_text(encoding="utf-8"))["files"]
    for entry in upstream:
        body = (OUTPUT / entry["stored_path"]).read_bytes()
        if sha256(body) != entry["sha256"] or len(body) != entry["byte_length"]:
            raise ValueError(f"Pinned upstream license changed: {entry['stored_path']}")

    packages: dict[str, dict] = {}
    platform_roles: dict[str, dict] = {}
    platform_features: dict[str, dict] = {}
    target_counts = {}
    for target in TARGETS:
        metadata = read_metadata(target, supplied, args.online)
        selected, roles = dependency_graph(metadata)
        nodes = {n["id"]: n for n in metadata["resolve"]["nodes"]}
        target_counts[target] = len(selected)
        for package_id, package in selected.items():
            key = package_key(package)
            packages[key] = package
            platform_roles.setdefault(key, {})[target] = sorted(roles[package_id])
            platform_features.setdefault(key, {})[target] = sorted(nodes[package_id]["features"])

    texts: dict[str, bytes] = {}
    records = []
    rune = next(p for p in packages.values() if p["name"] == "rune")
    rune_root = git_root(Path(rune["manifest_path"]).parent)
    rune_revision = rune["source"].rsplit("#", 1)[1]
    assets = []
    (OUTPUT / "licenses").mkdir(exist_ok=True)
    for output_name, source_path in RUNE_ASSETS.items():
        body = (rune_root / source_path).read_bytes()
        (OUTPUT / "licenses" / output_name).write_bytes(body)
        assets.append({"name": output_name, "path": "licenses/" + output_name, "source_path": source_path,
                       "source_repository": "https://github.com/anomalyco/rune", "git_revision": rune_revision,
                       "sha256": sha256(body), "byte_length": len(body)})

    for key, package in sorted(packages.items()):
        base = Path(package["manifest_path"]).parent
        source = package["source"]
        record = {"name": package["name"], "version": package["version"], "source": source or "workspace",
                  "license_expression": package["license"], "authors": package["authors"],
                  "repository": package["repository"], "targets": platform_roles[key],
                  "metadata_features": platform_features[key], "license_texts": [],
                  "manifest_sha256": sha256((base / "Cargo.toml").read_bytes())}
        vcs_file = base / ".cargo_vcs_info.json"
        vcs = json.loads(vcs_file.read_text()) if vcs_file.is_file() else {}
        if vcs:
            record["cargo_vcs_info"] = vcs
        if source and source.startswith("registry+"):
            expected = checksums.get((package["name"], package["version"], source))
            if not expected:
                raise ValueError(f"Locked checksum missing: {key}")
            record["registry_archive"] = archive_checksum(package, expected)
        if source and source.startswith("git+"):
            record["git_revision"] = source.rsplit("#", 1)[1]

        candidates = [(path, path.relative_to(base).as_posix(), "crate-archive") for path in license_files(base)]
        if package["license_file"]:
            license_file = Path(package["license_file"])
            if not license_file.is_absolute():
                license_file = base / license_file
            candidates.append((license_file, package["license_file"], "cargo-license-file"))
        if source and source.startswith("git+"):
            checkout = git_root(base)
            for path in sorted(checkout.iterdir()):
                if path.is_file() and LICENSE_NAME.search(path.name):
                    candidates.append((path, path.name, "git-repository-root"))
        if source is None:
            record["workspace_manifest"] = (base / "Cargo.toml").relative_to(NATIVE).as_posix()
            record["manifest_sha256"] = repository_text_sha256(base / "Cargo.toml")
            candidates.append((NATIVE.parent / "LICENSE", "LICENSE", "workspace-repository-root"))

        repo = github_repository(package["repository"])
        for entry in upstream:
            if github_repository(entry["repository"]) == repo and vcs.get("git", {}).get("sha1") == entry["git_revision"]:
                candidates.append((OUTPUT / entry["stored_path"], entry["source_path"], entry["retrieval_url"]))

        seen = set()
        for path, original_path, origin in candidates:
            body = path.read_bytes()
            body.decode("utf-8")
            digest = sha256(body)
            identity = (digest, original_path, origin)
            if identity in seen:
                continue
            seen.add(identity)
            texts[digest] = body
            record["license_texts"].append({"sha256": digest, "source_path": original_path, "origin": origin, "byte_length": len(body)})
        if not record["license_texts"]:
            raise ValueError(f"No actual license text found for {key}; add revision-pinned original source")
        records.append(record)

    # Preserve copyright headers for upstream files whose archive omitted its root licenses.
    # These are verbatim comments, with a source path; no substitute copyright is invented.
    copyright_headers = []
    for package in sorted(packages.values(), key=package_key):
        if not package["name"].startswith("accesskit"):
            continue
        base = Path(package["manifest_path"]).parent
        for path in sorted((base / "src").rglob("*.rs")):
            lines = path.read_text(encoding="utf-8").splitlines(keepends=True)
            header = []
            for line in lines:
                if line.startswith("//") or not line.strip():
                    header.append(line)
                else:
                    break
            value = "".join(header).strip()
            if "Copyright" in value:
                copyright_headers.append({"package": package["name"], "version": package["version"], "source_path": path.relative_to(base).as_posix(), "text": value})

    header = ["STANDARD NOTES RUNE — THIRD-PARTY NOTICES", "",
              "Exact upstream license texts for the union of the locked Linux x86_64 and Windows x86_64 MSVC non-dev Cargo graphs.",
              "Normal dependencies, proc macros and build tools are listed separately by role in SOURCE_MANIFEST.json.",
              "This conservative source inventory is not a claim that every listed package contributes machine code to the linked binary.",
              "Identical license texts are included once and referenced by their SHA-256 digest. No license alternative is selected by this inventory.", "",
              "PACKAGE INDEX", ""]
    for record in records:
        header.extend([f"{record['name']} {record['version']} — {record['license_expression']}",
                       f"Source: {record['source']}",
                       "License texts: " + ", ".join(sorted({ref["sha256"] for ref in record["license_texts"]})), ""])
    header.extend(["ADDITIONAL ORIGINAL COPYRIGHT HEADERS", ""])
    for entry in copyright_headers:
        header.extend([f"{entry['package']} {entry['version']} / {entry['source_path']}", entry["text"], ""])
    output = bytearray(("\n".join(header) + "\n").encode())
    text_locations = {}
    for digest, body in sorted(texts.items()):
        output.extend(("\n" + "=" * 80 + "\nLICENSE TEXT SHA-256: " + digest + "\n" + "=" * 80 + "\n").encode())
        text_locations[digest] = {"byte_offset": len(output), "byte_length": len(body)}
        output.extend(body)
        output.extend(b"\n")
    (OUTPUT / "THIRD_PARTY_NOTICES.txt").write_bytes(output)

    manifest = {"schema_version": 1, "generator": "native/scripts/generate-notices.py",
                "generator_sha256": repository_text_sha256(Path(__file__)), "root_package": ROOT_PACKAGE,
                "targets": list(TARGETS), "target_package_counts": target_counts,
                "package_count_union": len(records), "license_text_count": len(texts),
                "scope": "Cargo metadata non-dev graph closure, including separately labeled host build tools and proc macros. Metadata features may be unified across workspace targets; this is not linker reachability evidence.",
                "workspace": {"repository": "https://github.com/vitorhubdev/standardnotes-rune", "Cargo.lock_sha256": repository_text_sha256(NATIVE / "Cargo.lock"),
                              "Cargo.toml_sha256": repository_text_sha256(NATIVE / "Cargo.toml"),
                              "repository_text_hashes": "SHA-256 after CRLF-to-LF normalization; crate archives and original license texts are hashed as exact bytes.",
                              "source_revision": "Recorded for each built artifact by the release build, not inferred from an uncommitted working tree."},
                "embedded_assets": assets, "packages": records,
                "copyright_headers": copyright_headers, "license_text_locations": text_locations,
                "license_text_container": "THIRD_PARTY_NOTICES.txt",
                "license_text_container_sha256": sha256(bytes(output))}
    toolchain = NATIVE / "rust-toolchain.toml"
    if toolchain.is_file():
        manifest["workspace"]["rust-toolchain.toml_sha256"] = repository_text_sha256(toolchain)
        manifest["workspace"]["rust_toolchain"] = tomllib.loads(toolchain.read_text())
    write_json(OUTPUT / "SOURCE_MANIFEST.json", manifest)

    markdown = ["# Third-party notices", "", "Full, unmodified license texts and copyright notices are in [THIRD_PARTY_NOTICES.txt](THIRD_PARTY_NOTICES.txt).",
                "The source identities, locked archive checksums, exact Git revisions, target graph roles, and hashes of the individual license texts are in [SOURCE_MANIFEST.json](SOURCE_MANIFEST.json).",
                "", f"This inventory covers **{len(records)}** unique packages and **{len(texts)}** distinct license texts for the two release targets.",
                "It includes build tools and proc macros conservatively; it does not assert that all listed packages are linked into the executable.",
                "", "## Embedded Rune assets", "", "| Component | Original license | Pinned source |", "| --- | --- | --- |"]
    for asset in assets:
        markdown.append(f"| {asset['name']} | [{asset['name']}]({asset['path']}) | Rune `{rune_revision}` / `{asset['source_path']}` |")
    markdown.extend(["", "## Resolved package inventory", "", "Normal = normal dependency path. Host = build dependency or proc-macro path. A package can have both roles.",
                     "", "| Package | Version | Declared license expression | Linux | Windows |", "| --- | --- | --- | --- | --- |"])
    for record in records:
        def roles_for(target: str) -> str:
            return ", ".join("Host" if role == "host-build" else "Normal" for role in record["targets"].get(target, [])) or "—"
        markdown.append(f"| {record['name']} | {record['version']} | {record['license_expression']} | {roles_for(TARGETS[0])} | {roles_for(TARGETS[1])} |")
    markdown.extend(["", "## Scope", "", "System libraries and the Windows SDK/CRT are not Cargo packages and are not represented as crate licenses in this inventory. Binary build provenance must record their platform/toolchain versions separately.",
                     "Source code for the application remains under its repository license. The individual upstream terms above remain applicable to each dependency or embedded asset.", ""])
    (OUTPUT / "THIRD_PARTY_NOTICES.md").write_text("\n".join(markdown), encoding="utf-8", newline="\n")
    files = sorted(path for path in OUTPUT.rglob("*") if path.is_file() and path.name != "SHA256SUMS")
    (OUTPUT / "SHA256SUMS").write_text("".join(f"{sha256(path.read_bytes())}  {path.relative_to(OUTPUT).as_posix()}\n" for path in files), encoding="utf-8", newline="\n")
    print(f"Generated {len(records)} package records, {len(texts)} complete license texts; targets: {target_counts}")


if __name__ == "__main__":
    main()
