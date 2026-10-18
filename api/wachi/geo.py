from pyproj import Transformer

TO_UTM = Transformer.from_crs("EPSG:4326", "EPSG:32718", always_xy=True)
TO_LL = Transformer.from_crs("EPSG:32718", "EPSG:4326", always_xy=True)


def to_utm(lon, lat):
    return TO_UTM.transform(lon, lat)


def to_ll(x, y):
    return TO_LL.transform(x, y)
