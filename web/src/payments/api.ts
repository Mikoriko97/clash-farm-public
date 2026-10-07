// Typed wrappers over the payment endpoints (docs/API.md §Payments).
import type { ApiClient } from "../api/client";

export interface OrderQuote {
  order_id: string;
  item_id: string;
  qty: number;
  price_usd_cents: number;
  amount_clash: number;
  burn_amount: number;
  transfer_amount: number;
  clash_per_usd: string;
  quote_expires_at: string;
  memo: string;
  mint: string;
  decimals: number;
  treasury_token_account: string;
}

export type OrderStatus = "quoted" | "submitted" | "fulfilled" | "rejected" | "needs_review" | "expired";

export interface SubmitOutcome {
  status: "fulfilled" | "submitted" | "rejected" | "needs_review";
  order_id: string;
  amount_clash?: number;
  reason_code?: string;
}

export interface OrderStatusView {
  order_id: string;
  status: OrderStatus;
  reason_code: string | null;
  amount_clash: number;
}

export interface PaymentsApi {
  createOrder(itemId: string, qty: number): Promise<OrderQuote>;
  submit(orderId: string, signature: string): Promise<SubmitOutcome>;
  status(orderId: string): Promise<OrderStatusView>;
}

export function createPaymentsApi(client: ApiClient): PaymentsApi {
  return {
    createOrder: (itemId, qty) => client.post("/orders", { item_id: itemId, qty }),
    submit: (orderId, signature) => client.post(`/orders/${encodeURIComponent(orderId)}/submit`, { signature }),
    status: (orderId) => client.get(`/orders/${encodeURIComponent(orderId)}`),
  };
}
