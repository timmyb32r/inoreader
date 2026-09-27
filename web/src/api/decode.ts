import { wireSchemas } from "./generated";
type Schema = boolean | { [key: string]: any };
/** Validate at the network boundary before an unchecked value becomes UI state.
 * Never coerce, truncate, drop unknown fields, or include payloads in diagnostics. */
export function assertWire(
  name: keyof typeof wireSchemas,
  value: unknown,
): void {
  const root: Schema = wireSchemas[name];
  const check = (schema: Schema, input: unknown): boolean => {
    if (typeof schema === "boolean") return schema;
    if (
      schema.$ref &&
      !check((root as any).$defs[schema.$ref.split("/").at(-1)], input)
    )
      return false;
    if (schema.enum && !schema.enum.includes(input)) return false;
    if ("const" in schema && schema.const !== input) return false;
    if (schema.anyOf && !schema.anyOf.some((s: Schema) => check(s, input)))
      return false;
    if (
      schema.oneOf &&
      schema.oneOf.filter((s: Schema) => check(s, input)).length !== 1
    )
      return false;
    if (schema.allOf && !schema.allOf.every((s: Schema) => check(s, input)))
      return false;
    const kinds = Array.isArray(schema.type) ? schema.type : [schema.type];
    if (
      schema.type &&
      !kinds.some((kind: string) =>
        kind === "null"
          ? input === null
          : kind === "array"
            ? Array.isArray(input)
            : kind === "object"
              ? !!input && typeof input === "object" && !Array.isArray(input)
              : kind === "integer"
                ? Number.isSafeInteger(input)
                : kind === "number"
                  ? typeof input === "number" && Number.isFinite(input)
                  : typeof input === kind,
      )
    )
      return false;
    if (
      typeof input === "number" &&
      ((schema.minimum !== undefined && input < schema.minimum) ||
        (schema.maximum !== undefined && input > schema.maximum))
    )
      return false;
    if (
      Array.isArray(input) &&
      schema.items &&
      !input.every((item) => check(schema.items, item))
    )
      return false;
    if (input && typeof input === "object" && !Array.isArray(input)) {
      const record = input as Record<string, unknown>;
      if (schema.required?.some((key: string) => !(key in record)))
        return false;
      for (const [key, item] of Object.entries(record)) {
        const property = schema.properties?.[key];
        if (property !== undefined && !check(property, item)) return false;
        if (
          property === undefined &&
          schema.additionalProperties !== undefined &&
          !check(schema.additionalProperties, item)
        )
          return false;
      }
    }
    return true;
  };
  if (!check(root, value)) throw new Error(`Invalid API response (${name})`);
}
/** Every transport call names its response contract; URL matching is forbidden. */
export type ResponseContract =
  | keyof typeof wireSchemas
  | `${keyof typeof wireSchemas}[]`
  | "empty"
  | "string";
export function decodeResponse(
  contract: ResponseContract,
  value: unknown,
): unknown {
  if (contract === "empty") {
    if (value !== undefined) throw new Error("Invalid API response (empty)");
  } else if (contract === "string") {
    if (typeof value !== "string")
      throw new Error("Invalid API response (string)");
  } else if (contract.endsWith("[]")) {
    if (!Array.isArray(value))
      throw new Error(`Invalid API response (${contract})`);
    value.forEach((item) =>
      assertWire(contract.slice(0, -2) as keyof typeof wireSchemas, item),
    );
  } else assertWire(contract as keyof typeof wireSchemas, value);
  return value;
}
