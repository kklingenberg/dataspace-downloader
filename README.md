# Dataspace Downloader

A CLI tool that queries the Copernicus Dataspace [OpenSearch
APIs](https://documentation.dataspace.copernicus.eu/APIs/OpenSearch.html) for
products based on query parameters, and downloads said products from
[S3](https://documentation.dataspace.copernicus.eu/APIs/S3.html).

## Synopsis

```text
Query Copernicus Dataspace and download their assets from S3

Usage: dataspace-downloader [OPTIONS]

Options:
      --s3-endpoint-url <S3_ENDPOINT_URL>
          S3 endpoint URL, which defaults to https://eodata.dataspace.copernicus.eu/ [env: S3_ENDPOINT_URL=]
      --s3-access-key-id <S3_ACCESS_KEY_ID>
          S3 access key id [env: S3_ACCESS_KEY_ID=]
      --s3-secret-access-key <S3_SECRET_ACCESS_KEY>
          S3 secret_access key [env: S3_SECRET_ACCESS_KEY=]
  -k, --keys-file <KEYS_FILE>
          Keys file, optional; must be given if keys are not given inline [env: KEYS_FILE=]
  -c, --config <CONFIG>
          Configuration file (query parameters) [env: CONFIG=]
  -g, --geometry <GEOMETRY>
          File with geometry of interest (GeoJSON format) [env: GEOMETRY=]
  -o, --output <OUTPUT>
          The target directory where files will be downloaded; defaults to current directory [env: OUTPUT=]
  -p, --parallelism <PARALLELISM>
          Number of products to download in parallel [env: PARALLELISM=] [default: 5]
      --no-download
          Skip downloading, only list results
      --log-level <LOG_LEVEL>
          Logging verbosity level [env: LOG_LEVEL=] [default: INFO]
  -h, --help
          Print help
  -V, --version
          Print version

```

## Keys file format

The keys file may be used instead of inline --s3- parameters, or environment
variables. It should be a JSON-encoded file with the following fields:

```json
{
  "endpointUrl": "https://the-url-to-dataspace-s3-omit-if-unsure",
  "accessKeyId": "the-access-key-id",
  "secretAccessKey": "the-secret-access-key"
}
```

## Configuration file format

The configuration file should be used to pass on filters to the OpenSearch
request, to select the collection, and to filter the archive contents so that
only the required objects are downloaded. It should be a JSON-encoded file with
the following fields:

```json
{
  "endpointUrl": "https://the-url-to-dataspace-opensearch-omit-if-unsure",
  "collection": "a-collection-identifier",
  "query": {
    "fields": "used-to-filter-elements"
  },
  "depaginate": false,
  "globPatterns": ["**/*.jp2", "other-filters"]
}
```

The `query` field maps to whichever parameter can be used to filter the results
on DataSpace's end. For example, for the `Sentinel1` collection, the query
fields are described
[here](https://catalogue.dataspace.copernicus.eu/resto/api/collections/Sentinel1/describe.xml).

The `depaginate` field defaults to `false` and if set to `true` will cause the
downloader to "uncoil" the results in pages.

The `globPatterns` are filters that apply over the contents of each product
matched by the `query`. Each pattern is joined with the others with _OR_,
meaning only one pattern should match for a file to be downloaded. An exception
is the empty list of patterns, with is interpreted just like the match-all
pattern `**`.

## Geometry file

The `geometry` filter could be given as a field inside the `query` field in the
configuration file. An alternative is to provide a GeoJSON-formatted file with a
single feature as the input to `--geometry`. Doing so will insert the given
geometry as the `geometry` filter of the query, _overwriting the one specified
in the configuration file_.

## License

MIT
