export function naturalSort(a, b) {
    return a.filename.localeCompare(b.filename, undefined, {numeric: true, sensitivity: 'base'});
}
