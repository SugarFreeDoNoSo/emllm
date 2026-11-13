"""Corpus determinista con operandos F32 trazables y oráculos escalares F64."""

import math
import struct

import numpy as np

from prepare_tiny import sha256, write_json


def selected_indices(values):
    """Primeros cuatro, mínimo/máximo y menor magnitud no nula; desempate inicial."""
    values = np.asarray(values, dtype=np.float64).reshape(-1)
    if not len(values) or not np.isfinite(values).all():
        raise ValueError("empty_or_nonfinite_selection")
    indices = list(range(min(4, len(values))))
    indices.extend([int(values.argmin()), int(values.argmax())])
    nonzero = np.flatnonzero(values)
    if len(nonzero):
        indices.append(int(nonzero[np.abs(values[nonzero]).argmin()]))
    return sorted(set(indices))


def balanced_sum(values):
    """Árbol por pares adyacentes; el elemento impar pasa al siguiente nivel."""
    values = list(values)
    while len(values) > 1:
        values = [values[i] + values[i + 1] if i + 1 < len(values) else values[i]
                  for i in range(0, len(values), 2)]
    return values[0] if values else 0.0


def number(ref):
    return struct.unpack("<f", struct.pack("<I", ref["bits"]))[0]


class Corpus:
    def __init__(self, weights):
        self.weights = {name: value.numpy() for name, value in weights.items()}
        self.scalars = {}
        self.operations, self.reductions, self.components = [], [], []
        self.catalog = {}
        for name, values in self.weights.items():
            source = "package/" + name
            self.catalog[source] = {"shape": list(values.shape), "dtype": "<f4"}
            for index in range(values.size):
                self.ref(source, values, index)

    def ref(self, source, values, index):
        bits = int(np.asarray(values, dtype="<f4").reshape(-1).view("<u4")[index])
        ref = {"bits": bits, "source": source, "index": int(index)}
        self.scalars.setdefault(bits, ref)
        return ref

    def operation(self, family, op, args):
        numbers = [number(value) for value in args]
        a = numbers[0]
        if op == "add": value = a + numbers[1]
        elif op == "sub": value = a - numbers[1]
        elif op == "mul": value = a * numbers[1]
        elif op == "neg": value = -a
        elif op == "reciprocal": value = 1.0 / a
        elif op == "rsqrt": value = 1.0 / math.sqrt(a)
        elif op == "exp": value = math.exp(a)
        elif op == "silu": value = a / (1.0 + math.exp(-a))
        else: raise ValueError(op)
        if not math.isfinite(value):
            raise ValueError(f"nonfinite_oracle: {family}/{op}")
        self.operations.append({"family": family, "operation": op, "args": args, "expected_f64": value})

    def vector_refs(self, source, values, indices):
        return [self.ref(source, values, int(i)) for i in indices]

    def dot(self, family, left, right):
        products = [number(a) * number(b) for a, b in zip(left, right, strict=True)]
        self.reductions.append({"family": family, "operation": "dot", "length": len(left),
                                "left": left, "right": right, "expected_f64": balanced_sum(products)})
        indices = selected_indices(products)
        # Incluye ambos extremos de signo aun si la cancelación deja un resultado pequeño.
        for index in indices:
            self.operation(family, "mul", [left[index], right[index]])

    def add_fixture(self, fixture, trace):
        arrays = {name: tensor.numpy() for name, tensor in trace.items()}
        prefix = fixture + "/explicit_f32/"
        for name, value in arrays.items():
            source = prefix + name
            self.catalog[source] = {"shape": list(value.shape), "dtype": "<f4"}
            for index in selected_indices(value):
                self.ref(source, value, index)

        def refs(name, indices):
            return self.vector_refs(prefix + name, arrays[name], indices)

        def binary(family, op, lhs, rhs):
            a, b = arrays[lhs].reshape(-1), arrays[rhs].reshape(-1)
            if a.shape != b.shape:
                raise ValueError(f"shape_mismatch: {family}")
            result = a.astype(np.float64) + b if op == "add" else a.astype(np.float64) - b if op == "sub" else a.astype(np.float64) * b
            indices = set(selected_indices(a) + selected_indices(b) + selected_indices(result))
            if op in ("add", "sub"):
                scale = np.abs(a.astype(np.float64)) + np.abs(b.astype(np.float64))
                ratios = np.divide(np.abs(result), scale, out=np.full_like(scale, np.inf), where=scale != 0)
                indices.add(int(ratios.argmin()))
            for i in sorted(indices):
                self.operation(prefix + family, op, [refs(lhs, [i])[0], refs(rhs, [i])[0]])

        def unary(name, operation):
            for i in selected_indices(arrays[name]):
                self.operation(prefix + name, operation, refs(name, [i]))

        def linear(input_name, weight_name, output_name, last=False):
            x = arrays[input_name].reshape(-1, arrays[input_name].shape[-1])
            w = self.weights[weight_name]
            rows = x[-1:] if last else x
            # Selección sin BLAS: productos explícitos F64, también para medir
            # cancelación. El oráculo persistido usa balanced_sum, no esta suma.
            products = rows.astype(np.float64)[:, None, :] * w.astype(np.float64)[None, :, :]
            output = products.sum(axis=-1)
            scale = np.abs(products).sum(axis=-1)
            ratios = np.divide(np.abs(output), scale, out=np.full_like(scale, np.inf), where=scale != 0)
            indices = set(selected_indices(output)) | {int(ratios.argmin())}
            for index in sorted(indices):
                row, coordinate = divmod(index, w.shape[0])
                if last: row += len(x) - 1
                left = refs(input_name, range(row * w.shape[1], (row + 1) * w.shape[1]))
                right = self.vector_refs("package/" + weight_name, w, range(coordinate * w.shape[1], (coordinate + 1) * w.shape[1]))
                self.dot(prefix + output_name + f"[{row},{coordinate}]", left, right)

        def rms(input_name, norm_name, weight_name):
            x = arrays[input_name]
            w = self.weights[weight_name]
            rows = {0, len(x) - 1, int(np.abs(x).max(axis=-1).argmax())}
            epsilon = self.ref("package/norm.epsilon", self.weights["norm.epsilon"], 0)
            for row in sorted(rows):
                args = refs(input_name, range(row * 64, (row + 1) * 64))
                weight = self.vector_refs("package/" + weight_name, w, range(64))
                values = [number(arg) for arg in args]
                inverse = 1.0 / math.sqrt(balanced_sum(v * v for v in values) / 64 + number(epsilon))
                expected = [v * inverse * number(gain) for v, gain in zip(values, weight, strict=True)]
                self.components.append({"family": prefix + norm_name + f"[{row}]", "operation": "rmsnorm",
                                        "input": args, "weight": weight, "epsilon": epsilon, "expected_f64": expected})
                self.dot(prefix + norm_name + f".squares[{row}]", args, args)
            unary(norm_name + ".denominator", "rsqrt")
            for i in selected_indices(arrays[norm_name + ".mean"]):
                self.operation(prefix + norm_name, "add", [refs(norm_name + ".mean", [i])[0], epsilon])

        for layer in range(5):
            p = f"layers.{layer}."
            input_name = "embedding" if layer == 0 else f"layers.{layer - 1}.residual"
            rms(input_name, p + "attention_norm", p + "attention_norm.weight")
            for weight, output in [("wq", "query"), ("wk", "key"), ("wv", "value")]:
                linear(p + "attention_norm", p + f"attention.{weight}.weight", p + output)
            linear(p + "context", p + "attention.wo.weight", p + "attention_output")
            binary(p + "attention_residual", "add", input_name, p + "attention_output")
            rms(p + "attention_residual", p + "ffn_norm", p + "ffn_norm.weight")
            linear(p + "ffn_norm", p + "feed_forward.w1.weight", p + "gate")
            linear(p + "ffn_norm", p + "feed_forward.w3.weight", p + "up")
            linear(p + "gated", p + "feed_forward.w2.weight", p + "down")
            binary(p + "residual", "add", p + "attention_residual", p + "down")
            binary(p + "gated", "mul", p + "silu", p + "up")
            unary(p + "gate", "silu")
            unary(p + "gate", "neg")
            unary(p + "negative_gate", "exp")
            unary(p + "gate_denominator", "reciprocal")
            unary(p + "score_shifts", "exp")
            for rotated in ["query_rope", "key_rope"]:
                name = p + rotated
                binary(name + ".real", "sub", name + ".real_cos", name + ".imag_sin")
                binary(name + ".imag", "add", name + ".real_sin", name + ".imag_cos")
            scores = arrays[p + "scores"]
            t = arrays[p + "query"].shape[0]
            # Producto Q.K completo de dimensión ocho, incluidos extremos de score.
            for index in selected_indices(scores):
                head, packed = divmod(index, scores.shape[1])
                position = (math.isqrt(8 * packed + 1) - 1) // 2
                key = packed - position * (position + 1) // 2
                left = refs(p + "query_rope", range((position * 8 + head) * 8, (position * 8 + head + 1) * 8))
                right = refs(p + "key_rope", range((key * 4 + head // 2) * 8, (key * 4 + head // 2 + 1) * 8))
                self.dot(prefix + p + f"scores[{head},{position},{key}]", left, right)
            for head in range(8):
                for position in [0, t - 1]:
                    offset = head * scores.shape[1] + position * (position + 1) // 2
                    args = refs(p + "scores", range(offset, offset + position + 1))
                    values = [number(arg) for arg in args]
                    exponentials = [math.exp(value - values[0]) for value in values]
                    total = balanced_sum(exponentials)
                    self.components.append({"family": prefix + p + f"softmax[{head},{position}]", "operation": "softmax",
                                            "input": args, "pivot": 0, "expected_f64": [value / total for value in exponentials]})
                    for arg in args:
                        self.operation(prefix + p + "score_shift", "sub", [arg, args[0]])
        rms("layers.4.residual", "norm", "norm.weight")
        linear("norm", "output.weight", "logits", last=True)

    def save(self, directory, package_hash, fixtures):
        directory.mkdir(parents=True, exist_ok=True)
        ordered = [self.scalars[bits] for bits in sorted(self.scalars)]
        data = b"".join(struct.pack("<I", item["bits"]) for item in ordered)
        (directory / "scalars.u32").write_bytes(data)
        scalar_hash = write_json(directory / "scalars.json", {
            "schema_version": 1, "file": "scalars.u32", "sha256": sha256(data),
            "order": "unsigned bits ascending; first observed tensor/index retained per value",
            "values": ordered,
        })
        manifest = {
            "schema_version": 1, "package_sha256": package_hash,
            "traces": {name: fixture["traces"]["explicit_f32"] for name, fixture in fixtures.items()},
            "scalar_manifest_sha256": scalar_hash, "sources": self.catalog,
            "selection": "first four, min, max, smallest nonzero magnitude; first index breaks ties; minimum relative cancellation in residuals and linear outputs",
            "oracle": "Python binary64 on exact F32 operands; pairwise adjacent balanced reduction, odd tail carried",
            "operations": self.operations, "reductions": self.reductions, "components": self.components,
        }
        digest = write_json(directory / "corpus.json", manifest)
        return {"sha256": digest, "scalars_sha256": sha256(data), "distinct_scalars": len(ordered),
                "operations": len(self.operations), "reductions": len(self.reductions), "components": len(self.components)}
