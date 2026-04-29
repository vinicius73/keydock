import { KeydockValidationError } from "../errors.js";

export function validateNonNegativeInteger(name: string, value: number): void {
  if (!Number.isFinite(value) || !Number.isInteger(value) || value < 0) {
    throw new KeydockValidationError(`${name} must be a finite integer greater than or equal to 0`);
  }
}

export function validatePositiveInteger(name: string, value: number): void {
  if (!Number.isFinite(value) || !Number.isInteger(value) || value <= 0) {
    throw new KeydockValidationError(`${name} must be a positive integer`);
  }
}

export function validateTtlSeconds(ttlSeconds: number): void {
  validateNonNegativeInteger("ttlSeconds", ttlSeconds);
}
